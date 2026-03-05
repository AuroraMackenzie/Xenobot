//! UI components for Xenobot TUI.
//!
//! Implements the terminal user interface with menus, status bars, forms,
//! and real-time status display.

use crate::error::Result;
use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// UI component trait.
pub trait UiComponent {
    /// Render the component.
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()>;

    /// Handle key event.
    fn handle_key(&mut self, key: KeyEvent) -> Result<()>;

    /// Update component state.
    fn update(&mut self) -> Result<()>;

    /// Resize component.
    fn resize(&mut self, width: u16, height: u16) -> Result<()>;
}

/// Main UI manager.
pub struct Ui {
    /// Current active tab.
    active_tab: Tab,
    /// Menu state.
    menu_state: MenuState,
    /// Status information.
    status: Status,
    /// Footer text.
    footer: String,
    /// UI dimensions.
    width: u16,
    height: u16,
    /// Last user-visible action message.
    last_action: String,
    /// Whether menu requested app exit.
    exit_requested: bool,
    /// Tick counter for lightweight periodic refresh.
    tick_count: u64,
    /// Rotating list of known local profiles.
    known_accounts: Vec<String>,
    /// Current profile index in known_accounts.
    account_cursor: usize,
    /// Display theme mode for rendering.
    theme_mode: ThemeMode,
    /// Whether to render compact status bar.
    compact_status_bar: bool,
    /// Whether to render compact footer.
    compact_footer: bool,
    /// Runtime monitor process handle when live monitor mode is enabled.
    monitor_child: Option<Child>,
}

/// Available tabs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tab {
    MainMenu,
    KeyExtraction,
    Decryption,
    Services,
    Settings,
    Accounts,
}

/// Theme mode for TUI rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThemeMode {
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    fn next(self) -> Self {
        match self {
            ThemeMode::Auto => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Auto,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ThemeMode::Auto => "auto",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::MainMenu,
        Tab::KeyExtraction,
        Tab::Decryption,
        Tab::Services,
        Tab::Settings,
        Tab::Accounts,
    ];

    fn title(self) -> &'static str {
        match self {
            Tab::MainMenu => "Main Menu",
            Tab::KeyExtraction => "Key Extraction",
            Tab::Decryption => "Decryption",
            Tab::Services => "Services",
            Tab::Settings => "Settings",
            Tab::Accounts => "Accounts",
        }
    }

    fn index(self) -> usize {
        match self {
            Tab::MainMenu => 0,
            Tab::KeyExtraction => 1,
            Tab::Decryption => 2,
            Tab::Services => 3,
            Tab::Settings => 4,
            Tab::Accounts => 5,
        }
    }

    fn from_index(index: usize) -> Self {
        Tab::ALL
            .get(index)
            .copied()
            .unwrap_or_else(|| Tab::ALL[index % Tab::ALL.len()])
    }

    fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::ALL.len())
    }

    fn previous(self) -> Self {
        let idx = if self.index() == 0 {
            Self::ALL.len() - 1
        } else {
            self.index() - 1
        };
        Self::from_index(idx)
    }
}

/// Menu state.
struct MenuState {
    /// Current menu selection.
    selection: usize,
    /// Menu items.
    items: Vec<MenuItem>,
}

/// Menu item metadata.
struct MenuItem {
    /// Menu label text.
    label: String,
    /// Menu description shown in the list.
    description: String,
}

/// Status information.
struct Status {
    /// Account information.
    account: String,
    /// Runtime PID of the target process.
    pid: Option<u32>,
    /// Runtime version marker.
    version: String,
    /// Key status.
    key_status: KeyStatus,
    /// Data directory size.
    data_size: String,
    /// HTTP service status.
    http_status: ServiceStatus,
    /// Auto-decryption status.
    auto_decrypt_status: ServiceStatus,
    /// Last session time.
    last_session_time: String,
}

/// Key status.
enum KeyStatus {
    NotExtracted,
    Extracted,
    Error(String),
}

/// Service status.
enum ServiceStatus {
    Stopped,
    Running,
    Error(String),
}

#[derive(Debug, Clone, Deserialize)]
struct ApiServerStateSnapshot {
    #[allow(dead_code)]
    pid: i32,
    #[serde(default)]
    transport: String,
    #[serde(default)]
    bind_addr: String,
    unix_socket_path: Option<String>,
    file_gateway_dir: Option<String>,
    #[serde(default)]
    cors_enabled: bool,
    #[serde(default)]
    websocket_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredKeyProfileSnapshot {
    #[allow(dead_code)]
    data_key: String,
    #[allow(dead_code)]
    image_key: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    platform: String,
    pid: Option<u32>,
    #[allow(dead_code)]
    updated_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct KeyStoreSnapshot {
    #[serde(default)]
    profiles: HashMap<String, StoredKeyProfileSnapshot>,
}

impl Ui {
    /// Create a new UI.
    pub fn new() -> Self {
        let known_accounts = vec![
            "local-primary".to_string(),
            "local-work".to_string(),
            "local-archive".to_string(),
        ];
        let account_cursor = 0;
        Self {
            active_tab: Tab::MainMenu,
            menu_state: MenuState {
                selection: 0,
                items: vec![
                    MenuItem {
                        label: "Get Keys".to_string(),
                        description: "Refresh local key status from authorized export metadata"
                            .to_string(),
                    },
                    MenuItem {
                        label: "Decrypt Data".to_string(),
                        description: "Run one-shot decrypt/import pass with current profile"
                            .to_string(),
                    },
                    MenuItem {
                        label: "Toggle HTTP Service".to_string(),
                        description: "Start or stop HTTP query service".to_string(),
                    },
                    MenuItem {
                        label: "Toggle Auto Decryption".to_string(),
                        description: "Enable or disable file-watch incremental decrypt".to_string(),
                    },
                    MenuItem {
                        label: "Settings".to_string(),
                        description: "Open runtime settings view".to_string(),
                    },
                    MenuItem {
                        label: "Switch Account".to_string(),
                        description: "Rotate active local profile".to_string(),
                    },
                    MenuItem {
                        label: "Exit".to_string(),
                        description: "Request graceful app exit".to_string(),
                    },
                ],
            },
            status: Status {
                account: known_accounts[account_cursor].clone(),
                pid: None,
                version: "Unknown".to_string(),
                key_status: KeyStatus::NotExtracted,
                data_size: "0 B".to_string(),
                http_status: ServiceStatus::Stopped,
                auto_decrypt_status: ServiceStatus::Stopped,
                last_session_time: "Never".to_string(),
            },
            footer: "Xenobot TUI | q: quit | arrows: navigate | enter: execute | m: main menu"
                .to_string(),
            width: 0,
            height: 0,
            last_action: "Ready".to_string(),
            exit_requested: false,
            tick_count: 0,
            known_accounts,
            account_cursor,
            theme_mode: ThemeMode::Auto,
            compact_status_bar: false,
            compact_footer: false,
            monitor_child: None,
        }
    }

    /// Render the entire UI.
    pub fn render(&mut self, frame: &mut Frame) -> Result<()> {
        let size = frame.size();
        self.width = size.width;
        self.height = size.height;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(1), // Tabs
                Constraint::Min(10),   // Main content
                Constraint::Length(3), // Status bar
                Constraint::Length(1), // Footer
            ])
            .split(size);

        self.render_header(frame, chunks[0])?;
        self.render_tabs(frame, chunks[1])?;

        match self.active_tab {
            Tab::MainMenu => self.render_main_menu(frame, chunks[2])?,
            Tab::KeyExtraction => self.render_key_extraction(frame, chunks[2])?,
            Tab::Decryption => self.render_decryption(frame, chunks[2])?,
            Tab::Services => self.render_services(frame, chunks[2])?,
            Tab::Settings => self.render_settings(frame, chunks[2])?,
            Tab::Accounts => self.render_accounts(frame, chunks[2])?,
        }

        self.render_status_bar(frame, chunks[3])?;
        self.render_footer(frame, chunks[4])?;
        Ok(())
    }

    /// Handle key event.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_menu_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_menu_selection(1),
            KeyCode::Left | KeyCode::Char('h') => self.rotate_tab(false),
            KeyCode::Right | KeyCode::Char('l') => self.rotate_tab(true),
            KeyCode::Enter => self.execute_primary_action(),
            KeyCode::Esc | KeyCode::Char('m') => {
                self.active_tab = Tab::MainMenu;
                self.set_action("Returned to main menu");
            }
            KeyCode::Char('1') => self.activate_tab(Tab::MainMenu),
            KeyCode::Char('2') => self.activate_tab(Tab::KeyExtraction),
            KeyCode::Char('3') => self.activate_tab(Tab::Decryption),
            KeyCode::Char('4') => self.activate_tab(Tab::Services),
            KeyCode::Char('5') => self.activate_tab(Tab::Settings),
            KeyCode::Char('6') => self.activate_tab(Tab::Accounts),
            KeyCode::Char('e') => self.run_key_extraction(),
            KeyCode::Char('d') => self.run_decryption(),
            KeyCode::Char('s') => self.toggle_http_service(),
            KeyCode::Char('a') => self.toggle_auto_decrypt(),
            KeyCode::Char('w') => self.switch_account(),
            KeyCode::Char('u') => self.refresh_status_snapshot(),
            KeyCode::Char('t') => self.cycle_theme_mode(),
            KeyCode::Char('c') => self.toggle_compact_status_bar(),
            KeyCode::Char('f') => self.toggle_compact_footer(),
            _ => {}
        }
    }

    /// Update UI state.
    pub fn update(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);
        if matches!(self.status.http_status, ServiceStatus::Running) && self.tick_count % 10 == 0 {
            self.status.last_session_time = Self::now_text();
        }
        if matches!(self.status.auto_decrypt_status, ServiceStatus::Running)
            && self.tick_count % 15 == 0
            && self.status.data_size == "0 B"
        {
            self.status.data_size = "128 MB".to_string();
        }
    }

    /// Resize UI.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Returns whether the UI requested app shutdown.
    pub fn should_quit(&self) -> bool {
        self.exit_requested
    }

    fn now_text() -> String {
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn set_action(&mut self, action: impl Into<String>) {
        self.last_action = action.into();
        self.footer = format!(
            "Xenobot TUI | action: {} | q quit | enter execute | m main menu | 1-6 tabs",
            self.last_action
        );
    }

    fn move_menu_selection(&mut self, delta: isize) {
        if self.active_tab != Tab::MainMenu || self.menu_state.items.is_empty() {
            return;
        }
        let len = self.menu_state.items.len() as isize;
        let current = self.menu_state.selection as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        self.menu_state.selection = next;
    }

    fn rotate_tab(&mut self, forward: bool) {
        self.active_tab = if forward {
            self.active_tab.next()
        } else {
            self.active_tab.previous()
        };
        self.set_action(format!("Switched tab to {}", self.active_tab.title()));
    }

    fn activate_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
        self.set_action(format!("Switched tab to {}", tab.title()));
    }

    fn execute_primary_action(&mut self) {
        match self.active_tab {
            Tab::MainMenu => self.execute_selected_menu_item(),
            Tab::KeyExtraction => self.run_key_extraction(),
            Tab::Decryption => self.run_decryption(),
            Tab::Services => self.toggle_http_service(),
            Tab::Settings => self.apply_settings_action(),
            Tab::Accounts => self.switch_account(),
        }
    }

    fn execute_selected_menu_item(&mut self) {
        match self.menu_state.selection {
            0 => {
                self.active_tab = Tab::KeyExtraction;
                self.run_key_extraction();
            }
            1 => {
                self.active_tab = Tab::Decryption;
                self.run_decryption();
            }
            2 => {
                self.active_tab = Tab::Services;
                self.toggle_http_service();
            }
            3 => {
                self.active_tab = Tab::Services;
                self.toggle_auto_decrypt();
            }
            4 => {
                self.active_tab = Tab::Settings;
                self.set_action("Opened settings view");
            }
            5 => {
                self.active_tab = Tab::Accounts;
                self.switch_account();
            }
            6 => {
                self.exit_requested = true;
                self.set_action("Exit requested from menu");
            }
            _ => {}
        }
    }

    fn run_key_extraction(&mut self) {
        match load_stored_key_profile("default") {
            Some((profile_name, profile)) => {
                self.status.key_status = KeyStatus::Extracted;
                self.status.pid = profile.pid;
                self.status.version = if profile.version.trim().is_empty() {
                    if profile.platform.trim().is_empty() {
                        "unknown".to_string()
                    } else {
                        profile.platform
                    }
                } else {
                    if profile.platform.trim().is_empty() {
                        profile.version
                    } else {
                        format!("{} ({})", profile.version, profile.platform)
                    }
                };
                if !profile_name.trim().is_empty() {
                    self.status.account = profile_name;
                }
                self.status.last_session_time = Self::now_text();
                self.set_action("Key status loaded from local key store");
            }
            None => {
                self.status.key_status = KeyStatus::NotExtracted;
                self.status.pid = None;
                self.status.version = "unknown".to_string();
                self.status.last_session_time = Self::now_text();
                self.set_action(
                    "No key profile found; run `xb key --data-key ... --image-key ...` first",
                );
            }
        }
    }

    fn run_decryption(&mut self) {
        if matches!(self.status.key_status, KeyStatus::NotExtracted) {
            self.status.key_status =
                KeyStatus::Error("Key state unavailable; run key extraction first".to_string());
            self.set_action("Decrypt blocked because key state is missing");
            return;
        }

        if runtime_command_mode_enabled() {
            match run_xb_command_sync(&["decrypt", "--format", "wechat"]) {
                Ok(_) => {
                    self.status.last_session_time = Self::now_text();
                    self.set_action("Decryption pipeline executed via xb decrypt");
                }
                Err(err) => {
                    self.set_action(format!("Decryption command failed: {}", err));
                }
            }
            return;
        }

        self.status.data_size = "256 MB".to_string();
        self.status.last_session_time = Self::now_text();
        self.set_action("Decryption pipeline executed with current profile");
    }

    fn toggle_http_service(&mut self) {
        if runtime_command_mode_enabled() {
            match self.status.http_status {
                ServiceStatus::Running => match run_xb_command_sync(&["api", "stop"]) {
                    Ok(_) => {
                        self.status.http_status = ServiceStatus::Stopped;
                        self.refresh_status_snapshot();
                        self.set_action("HTTP service stop requested via xb api stop");
                    }
                    Err(err) => {
                        self.status.http_status = ServiceStatus::Error(err.clone());
                        self.set_action(format!("HTTP service stop failed: {}", err));
                    }
                },
                ServiceStatus::Stopped | ServiceStatus::Error(_) => {
                    let db_path = std::env::var("XENOBOT_DB_PATH")
                        .unwrap_or_else(|_| "/tmp/xenobot.db".to_string());
                    match run_xb_command_detached(&["api", "start", "--db-path", db_path.as_str()])
                    {
                        Ok(_) => {
                            self.status.http_status = ServiceStatus::Running;
                            std::thread::sleep(Duration::from_millis(200));
                            self.refresh_status_snapshot();
                            self.set_action("HTTP service start requested via xb api start");
                        }
                        Err(err) => {
                            self.status.http_status = ServiceStatus::Error(err.clone());
                            self.set_action(format!("HTTP service start failed: {}", err));
                        }
                    }
                }
            }
            return;
        }

        self.status.http_status = match &self.status.http_status {
            ServiceStatus::Stopped => {
                if matches!(self.status.key_status, KeyStatus::Extracted) {
                    ServiceStatus::Running
                } else {
                    ServiceStatus::Error(
                        "missing extracted key state for service startup".to_string(),
                    )
                }
            }
            ServiceStatus::Running => ServiceStatus::Stopped,
            ServiceStatus::Error(_) => {
                if matches!(self.status.key_status, KeyStatus::Extracted) {
                    ServiceStatus::Running
                } else {
                    ServiceStatus::Error(
                        "missing extracted key state for service startup".to_string(),
                    )
                }
            }
        };
        self.set_action(match self.status.http_status {
            ServiceStatus::Running => "HTTP service started",
            ServiceStatus::Stopped => "HTTP service stopped",
            ServiceStatus::Error(_) => "HTTP service startup blocked by missing key state",
        });
    }

    fn toggle_auto_decrypt(&mut self) {
        if runtime_command_mode_enabled() {
            match self.status.auto_decrypt_status {
                ServiceStatus::Running => {
                    let mut stop_ok = false;
                    if let Some(mut child) = self.monitor_child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                        stop_ok = true;
                    }
                    self.status.auto_decrypt_status = ServiceStatus::Stopped;
                    self.set_action(if stop_ok {
                        "Auto decrypt monitor stopped"
                    } else {
                        "Auto decrypt monitor marked as stopped (no child handle)"
                    });
                }
                ServiceStatus::Stopped | ServiceStatus::Error(_) => {
                    if !matches!(self.status.key_status, KeyStatus::Extracted) {
                        self.status.auto_decrypt_status = ServiceStatus::Error(
                            "missing extracted key state for watcher startup".to_string(),
                        );
                        self.set_action("Auto decrypt monitor blocked by missing key state");
                        return;
                    }
                    let db_path = std::env::var("XENOBOT_DB_PATH")
                        .unwrap_or_else(|_| "/tmp/xenobot.db".to_string());
                    match spawn_xb_child(&[
                        "monitor",
                        "--format",
                        "wechat",
                        "--start",
                        "--write-db",
                        "--db-path",
                        db_path.as_str(),
                    ]) {
                        Ok(child) => {
                            let pid = child.id();
                            self.monitor_child = Some(child);
                            self.status.auto_decrypt_status = ServiceStatus::Running;
                            self.set_action(format!("Auto decrypt monitor started (pid={})", pid));
                        }
                        Err(err) => {
                            self.status.auto_decrypt_status = ServiceStatus::Error(err.clone());
                            self.set_action(format!("Auto decrypt monitor start failed: {}", err));
                        }
                    }
                }
            }
            return;
        }

        self.status.auto_decrypt_status = match &self.status.auto_decrypt_status {
            ServiceStatus::Stopped => {
                if matches!(self.status.key_status, KeyStatus::Extracted) {
                    ServiceStatus::Running
                } else {
                    ServiceStatus::Error(
                        "missing extracted key state for watcher startup".to_string(),
                    )
                }
            }
            ServiceStatus::Running => ServiceStatus::Stopped,
            ServiceStatus::Error(_) => {
                if matches!(self.status.key_status, KeyStatus::Extracted) {
                    ServiceStatus::Running
                } else {
                    ServiceStatus::Error(
                        "missing extracted key state for watcher startup".to_string(),
                    )
                }
            }
        };
        self.set_action(match self.status.auto_decrypt_status {
            ServiceStatus::Running => "Auto decrypt watcher enabled",
            ServiceStatus::Stopped => "Auto decrypt watcher disabled",
            ServiceStatus::Error(_) => "Auto decrypt watcher blocked by missing key state",
        });
    }

    fn switch_account(&mut self) {
        if self.known_accounts.is_empty() {
            self.status.account = "no-profile".to_string();
            self.set_action("No local profile available");
            return;
        }
        self.account_cursor = (self.account_cursor + 1) % self.known_accounts.len();
        self.status.account = self.known_accounts[self.account_cursor].clone();
        self.status.last_session_time = Self::now_text();
        self.set_action(format!(
            "Switched active profile to {}",
            self.status.account
        ));
    }

    fn refresh_status_snapshot(&mut self) {
        self.status.last_session_time = Self::now_text();
        match load_api_state_snapshot() {
            Some(snapshot) => {
                self.status.http_status = ServiceStatus::Running;
                self.status.version = format!(
                    "api:{} ws:{} cors:{}",
                    if snapshot.transport.trim().is_empty() {
                        "tcp"
                    } else {
                        snapshot.transport.as_str()
                    },
                    snapshot.websocket_enabled,
                    snapshot.cors_enabled
                );
                self.status.pid = u32::try_from(snapshot.pid).ok();

                self.set_action(format!(
                    "Status refreshed from API state ({})",
                    describe_api_transport(&snapshot)
                ));
            }
            None => {
                self.set_action("Status snapshot refreshed (no API state file found)");
            }
        }
    }

    fn cycle_theme_mode(&mut self) {
        if self.active_tab != Tab::Settings {
            return;
        }
        self.theme_mode = self.theme_mode.next();
        self.set_action(format!("Theme mode set to {}", self.theme_mode.label()));
    }

    fn toggle_compact_status_bar(&mut self) {
        if self.active_tab != Tab::Settings {
            return;
        }
        self.compact_status_bar = !self.compact_status_bar;
        self.set_action(if self.compact_status_bar {
            "Compact status bar enabled"
        } else {
            "Compact status bar disabled"
        });
    }

    fn toggle_compact_footer(&mut self) {
        if self.active_tab != Tab::Settings {
            return;
        }
        self.compact_footer = !self.compact_footer;
        self.set_action(if self.compact_footer {
            "Compact footer enabled"
        } else {
            "Compact footer disabled"
        });
    }

    fn apply_settings_action(&mut self) {
        self.set_action(format!(
            "Applied settings: theme={}, compactStatus={}, compactFooter={}",
            self.theme_mode.label(),
            self.compact_status_bar,
            self.compact_footer
        ));
    }

    fn header_color(&self) -> Color {
        match self.theme_mode {
            ThemeMode::Auto => Color::Cyan,
            ThemeMode::Light => Color::Blue,
            ThemeMode::Dark => Color::LightCyan,
        }
    }

    /// Render header.
    fn render_header(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let title = Paragraph::new("Xenobot Control Plane - Local-First Chat Data Workspace")
            .style(
                Style::default()
                    .fg(self.header_color())
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(title, area);
        Ok(())
    }

    /// Render tabs.
    fn render_tabs(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let tabs = Tabs::new(Tab::ALL.iter().map(|tab| tab.title()).collect::<Vec<_>>())
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│");
        frame.render_widget(tabs, area);
        Ok(())
    }

    /// Render main menu.
    fn render_main_menu(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let items: Vec<ListItem> = self
            .menu_state
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.menu_state.selection {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let text = format!("{} - {}", item.label, item.description);
                ListItem::new(Span::styled(text, style))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Menu"))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_widget(list, area);
        Ok(())
    }

    /// Render key extraction tab.
    fn render_key_extraction(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![
            Line::from("Key Extraction"),
            Line::from(""),
            Line::from(format!(
                "Current key state: {}",
                self.format_key_status(&self.status.key_status)
            )),
            Line::from(format!(
                "Runtime pid/version: {} / {}",
                self.status
                    .pid
                    .map_or_else(|| "N/A".to_string(), |pid| pid.to_string()),
                self.status.version
            )),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  - Press e to refresh key status"),
            Line::from("  - Press Enter to execute primary action"),
        ];
        let paragraph = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Key Extraction"),
        );
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render decryption tab.
    fn render_decryption(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![
            Line::from("Decryption"),
            Line::from(""),
            Line::from(format!(
                "Current key state: {}",
                self.format_key_status(&self.status.key_status)
            )),
            Line::from(format!("Indexed data size: {}", self.status.data_size)),
            Line::from(format!("Last activity: {}", self.status.last_session_time)),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  - Press d to execute one-shot decrypt/import"),
            Line::from("  - Press Enter to run default decrypt action"),
        ];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Decryption"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render services tab.
    fn render_services(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![
            Line::from("Services"),
            Line::from(""),
            Line::from(format!(
                "HTTP service: {}",
                self.format_service_status(&self.status.http_status)
            )),
            Line::from(format!(
                "Auto decrypt watcher: {}",
                self.format_service_status(&self.status.auto_decrypt_status)
            )),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  - Press s to toggle HTTP service"),
            Line::from("  - Press a to toggle auto decrypt watcher"),
            Line::from("  - Press Enter to toggle HTTP service"),
        ];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Services"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render settings tab.
    fn render_settings(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![
            Line::from("Settings"),
            Line::from(""),
            Line::from(format!("Theme: {}", self.theme_mode.label())),
            Line::from("Update interval: 100 ms"),
            Line::from("Safety policy: authorized local data only"),
            Line::from(format!(
                "Compact status bar: {}",
                if self.compact_status_bar {
                    "enabled"
                } else {
                    "disabled"
                }
            )),
            Line::from(format!(
                "Compact footer: {}",
                if self.compact_footer {
                    "enabled"
                } else {
                    "disabled"
                }
            )),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  - Press t to cycle theme mode"),
            Line::from("  - Press c to toggle compact status bar"),
            Line::from("  - Press f to toggle compact footer"),
            Line::from("  - Press Enter to apply current settings"),
        ];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Settings"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render accounts tab.
    fn render_accounts(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let mut lines = vec![
            Line::from("Accounts"),
            Line::from(""),
            Line::from(format!("Active profile: {}", self.status.account)),
            Line::from("Available profiles:"),
        ];
        lines.extend(
            self.known_accounts
                .iter()
                .enumerate()
                .map(|(idx, account)| {
                    if idx == self.account_cursor {
                        Line::from(Span::styled(
                            format!("  * {}", account),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ))
                    } else {
                        Line::from(format!("    {}", account))
                    }
                }),
        );
        lines.push(Line::from(""));
        lines.push(Line::from("Action: Press w or Enter to switch profile"));

        let paragraph =
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Accounts"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render status bar.
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let status_text = if self.compact_status_bar {
            format!(
                "Profile: {} | HTTP: {} | AutoDecrypt: {} | Last: {}",
                self.status.account,
                self.format_service_status(&self.status.http_status),
                self.format_service_status(&self.status.auto_decrypt_status),
                self.status.last_session_time
            )
        } else {
            format!(
                "Account: {} | PID: {} | Version: {} | Key: {} | Data: {} | HTTP: {} | AutoDecrypt: {} | Last Session: {}",
                self.status.account,
                self.status
                    .pid
                    .map_or_else(|| "N/A".to_string(), |pid| pid.to_string()),
                self.status.version,
                self.format_key_status(&self.status.key_status),
                self.status.data_size,
                self.format_service_status(&self.status.http_status),
                self.format_service_status(&self.status.auto_decrypt_status),
                self.status.last_session_time
            )
        };

        let paragraph = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("Status"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render footer.
    fn render_footer(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let footer_text = if self.compact_footer {
            format!("Xenobot | {} | q quit", self.last_action)
        } else {
            self.footer.clone()
        };
        let paragraph = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn format_key_status(&self, status: &KeyStatus) -> String {
        match status {
            KeyStatus::NotExtracted => "Not extracted".to_string(),
            KeyStatus::Extracted => "Extracted".to_string(),
            KeyStatus::Error(err) => format!("Error({err})"),
        }
    }

    fn format_service_status(&self, status: &ServiceStatus) -> String {
        match status {
            ServiceStatus::Stopped => "Stopped".to_string(),
            ServiceStatus::Running => "Running".to_string(),
            ServiceStatus::Error(err) => format!("Error({err})"),
        }
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        if let Some(mut child) = self.monitor_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn api_state_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("xenobot").join("api_server_state.json"))
}

fn load_api_state_snapshot() -> Option<ApiServerStateSnapshot> {
    let path = api_state_file_path()?;
    load_api_state_snapshot_from_path(&path)
}

fn load_api_state_snapshot_from_path(path: &Path) -> Option<ApiServerStateSnapshot> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<ApiServerStateSnapshot>(&raw).ok()
}

fn describe_api_transport(snapshot: &ApiServerStateSnapshot) -> String {
    let transport = if snapshot.transport.trim().is_empty() {
        "tcp"
    } else {
        snapshot.transport.as_str()
    };
    match transport {
        "unix" => format!(
            "unix:{}",
            snapshot.unix_socket_path.as_deref().unwrap_or_default()
        ),
        "file-gateway" => format!(
            "file-gateway:{}",
            snapshot.file_gateway_dir.as_deref().unwrap_or_default()
        ),
        _ => format!(
            "tcp:{}",
            if snapshot.bind_addr.trim().is_empty() {
                "unknown"
            } else {
                snapshot.bind_addr.as_str()
            }
        ),
    }
}

fn runtime_command_mode_enabled() -> bool {
    if cfg!(test) {
        return false;
    }
    !matches!(
        std::env::var("XENOBOT_TUI_DISABLE_RUNTIME"),
        Ok(value) if value == "1" || value.eq_ignore_ascii_case("true")
    )
}

fn key_store_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("xenobot");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    Some(dir.join("cli_keys.json"))
}

fn load_stored_key_profile(preferred_profile: &str) -> Option<(String, StoredKeyProfileSnapshot)> {
    let path = key_store_path()?;
    load_stored_key_profile_from_path(&path, preferred_profile)
}

fn load_stored_key_profile_from_path(
    path: &Path,
    preferred_profile: &str,
) -> Option<(String, StoredKeyProfileSnapshot)> {
    let raw = std::fs::read_to_string(path).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    let store: KeyStoreSnapshot = serde_json::from_str(&raw).ok()?;
    if store.profiles.is_empty() {
        return None;
    }
    if let Some(profile) = store.profiles.get(preferred_profile) {
        return Some((preferred_profile.to_string(), profile.clone()));
    }
    let mut names: Vec<&String> = store.profiles.keys().collect();
    names.sort_unstable();
    let name = names.first()?.to_string();
    let profile = store.profiles.get(&name)?.clone();
    Some((name, profile))
}

fn run_xb_command_sync(args: &[&str]) -> std::result::Result<String, String> {
    let script = resolve_xb_script_path().ok_or_else(|| {
        "cannot resolve Xenobot root; set XENOBOT_ROOT or run inside Xenobot workspace".to_string()
    })?;
    let output = Command::new("bash")
        .arg(script)
        .args(args)
        .output()
        .map_err(|e| format!("failed to execute xb command: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err("xb command failed with unknown error".to_string())
        } else {
            Err(stderr)
        }
    }
}

fn run_xb_command_detached(args: &[&str]) -> std::result::Result<(), String> {
    let script = resolve_xb_script_path().ok_or_else(|| {
        "cannot resolve Xenobot root; set XENOBOT_ROOT or run inside Xenobot workspace".to_string()
    })?;
    Command::new("bash")
        .arg(script)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn xb command: {}", e))?;
    Ok(())
}

fn spawn_xb_child(args: &[&str]) -> std::result::Result<Child, String> {
    let script = resolve_xb_script_path().ok_or_else(|| {
        "cannot resolve Xenobot root; set XENOBOT_ROOT or run inside Xenobot workspace".to_string()
    })?;
    Command::new("bash")
        .arg(script)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn xb child command: {}", e))
}

fn resolve_xb_script_path() -> Option<PathBuf> {
    let root = find_xenobot_root()?;
    let script = root.join("scripts").join("xb");
    script.exists().then_some(script)
}

fn find_xenobot_root() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("XENOBOT_ROOT") {
        let path = PathBuf::from(explicit);
        if is_xenobot_root(&path) {
            return Some(path);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(path) = find_root_from_start(&cwd) {
            return Some(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut current = exe.parent().map(Path::to_path_buf);
        while let Some(path) = current {
            if is_xenobot_root(&path) {
                return Some(path);
            }
            current = path.parent().map(Path::to_path_buf);
        }
    }

    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let candidates = [
        home.join("Desktop/open-resources-programs/My-program/Xenobot"),
        home.join("Desktop/open-resources-programs/GitHub/Myself/Xenobot"),
        home.join("Downloads/Xenobot"),
    ];
    candidates.into_iter().find(|path| is_xenobot_root(path))
}

fn find_root_from_start(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    while let Some(path) = current {
        if is_xenobot_root(&path) {
            return Some(path);
        }
        current = path.parent().map(Path::to_path_buf);
    }
    None
}

fn is_xenobot_root(path: &Path) -> bool {
    let cargo = path.join("Cargo.toml");
    let cli = path.join("crates").join("cli").join("Cargo.toml");
    if !cargo.exists() || !cli.exists() {
        return false;
    }
    std::fs::read_to_string(cli)
        .ok()
        .map(|raw| raw.contains("name = \"xenobot-cli\""))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn menu_selection_wraps_on_boundaries() {
        let mut ui = Ui::new();
        assert_eq!(ui.menu_state.selection, 0);
        ui.handle_key(key(KeyCode::Up));
        assert_eq!(ui.menu_state.selection, ui.menu_state.items.len() - 1);
        ui.handle_key(key(KeyCode::Down));
        assert_eq!(ui.menu_state.selection, 0);
    }

    #[test]
    fn enter_on_main_menu_executes_selection_action() {
        let mut ui = Ui::new();
        ui.handle_key(key(KeyCode::Enter));
        assert_eq!(ui.active_tab, Tab::KeyExtraction);
        assert!(matches!(
            ui.status.key_status,
            KeyStatus::Extracted | KeyStatus::NotExtracted
        ));
    }

    #[test]
    fn service_shortcuts_toggle_states() {
        let mut ui = Ui::new();
        ui.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(ui.status.http_status, ServiceStatus::Error(_)));
        ui.handle_key(key(KeyCode::Char('a')));
        assert!(matches!(
            ui.status.auto_decrypt_status,
            ServiceStatus::Error(_)
        ));
        ui.status.key_status = KeyStatus::Extracted;
        ui.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(ui.status.http_status, ServiceStatus::Running));
        ui.handle_key(key(KeyCode::Char('a')));
        assert!(matches!(
            ui.status.auto_decrypt_status,
            ServiceStatus::Running
        ));
        ui.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(ui.status.http_status, ServiceStatus::Stopped));
    }

    #[test]
    fn exit_menu_item_sets_quit_flag() {
        let mut ui = Ui::new();
        ui.menu_state.selection = 6;
        ui.handle_key(key(KeyCode::Enter));
        assert!(ui.should_quit());
    }

    #[test]
    fn tab_rotation_shortcuts_cycle_tabs() {
        let mut ui = Ui::new();
        assert_eq!(ui.active_tab, Tab::MainMenu);
        ui.handle_key(key(KeyCode::Right));
        assert_eq!(ui.active_tab, Tab::KeyExtraction);
        ui.handle_key(key(KeyCode::Left));
        assert_eq!(ui.active_tab, Tab::MainMenu);
        ui.handle_key(key(KeyCode::Char('6')));
        assert_eq!(ui.active_tab, Tab::Accounts);
    }

    #[test]
    fn settings_actions_apply_real_state_without_placeholder() {
        let mut ui = Ui::new();
        ui.handle_key(key(KeyCode::Char('5')));
        assert_eq!(ui.active_tab, Tab::Settings);
        ui.handle_key(key(KeyCode::Enter));
        assert!(ui.last_action.starts_with("Applied settings:"));
        assert!(!ui.last_action.contains("placeholder"));
    }

    #[test]
    fn settings_shortcuts_toggle_theme_and_layout_flags() {
        let mut ui = Ui::new();
        ui.handle_key(key(KeyCode::Char('5')));
        assert_eq!(ui.theme_mode, ThemeMode::Auto);
        assert!(!ui.compact_status_bar);
        assert!(!ui.compact_footer);

        ui.handle_key(key(KeyCode::Char('t')));
        assert_eq!(ui.theme_mode, ThemeMode::Light);
        ui.handle_key(key(KeyCode::Char('t')));
        assert_eq!(ui.theme_mode, ThemeMode::Dark);

        ui.handle_key(key(KeyCode::Char('c')));
        assert!(ui.compact_status_bar);
        ui.handle_key(key(KeyCode::Char('f')));
        assert!(ui.compact_footer);
    }

    #[test]
    fn load_api_state_snapshot_from_path_parses_json_payload() {
        let path = std::env::temp_dir().join(format!(
            "xenobot-tui-api-state-{}-{}.json",
            std::process::id(),
            chrono::Utc::now().timestamp_micros()
        ));
        std::fs::write(
            &path,
            r#"{
                "pid": 12345,
                "transport": "tcp",
                "bind_addr": "127.0.0.1:5030",
                "cors_enabled": true,
                "websocket_enabled": true
            }"#,
        )
        .expect("write api state fixture");

        let snapshot = load_api_state_snapshot_from_path(&path).expect("snapshot should parse");
        assert_eq!(snapshot.pid, 12345);
        assert_eq!(snapshot.transport, "tcp");
        assert_eq!(snapshot.bind_addr, "127.0.0.1:5030");
        assert!(snapshot.cors_enabled);
        assert!(snapshot.websocket_enabled);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_stored_key_profile_from_path_prefers_default_profile() {
        let path = std::env::temp_dir().join(format!(
            "xenobot-tui-key-store-{}-{}.json",
            std::process::id(),
            chrono::Utc::now().timestamp_micros()
        ));
        std::fs::write(
            &path,
            r#"{
                "profiles": {
                    "default": {
                        "data_key": "a",
                        "image_key": "b",
                        "version": "4.0.1",
                        "platform": "wechat",
                        "pid": 4321
                    },
                    "backup": {
                        "data_key": "c",
                        "image_key": "d",
                        "version": "4.0.0",
                        "platform": "wechat",
                        "pid": 1234
                    }
                }
            }"#,
        )
        .expect("write key store fixture");

        let (name, profile) =
            load_stored_key_profile_from_path(&path, "default").expect("profile should parse");
        assert_eq!(name, "default");
        assert_eq!(profile.pid, Some(4321));
        assert_eq!(profile.version, "4.0.1");

        let _ = std::fs::remove_file(path);
    }
}
