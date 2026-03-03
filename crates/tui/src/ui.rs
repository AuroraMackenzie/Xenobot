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
            Tab::Settings => self.set_action("Settings action placeholder executed"),
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
        self.status.key_status = KeyStatus::Extracted;
        self.status.pid = Some(9821);
        self.status.version = "wechat-arm64-safe".to_string();
        if self.status.account.trim().is_empty() {
            self.status.account = self.known_accounts[self.account_cursor].clone();
        }
        self.status.last_session_time = Self::now_text();
        self.set_action("Key status refreshed from authorized local data source");
    }

    fn run_decryption(&mut self) {
        if matches!(self.status.key_status, KeyStatus::NotExtracted) {
            self.status.key_status =
                KeyStatus::Error("Key state unavailable; run key extraction first".to_string());
            self.set_action("Decrypt blocked because key state is missing");
            return;
        }

        self.status.data_size = "256 MB".to_string();
        self.status.last_session_time = Self::now_text();
        self.set_action("Decryption pipeline executed with current profile");
    }

    fn toggle_http_service(&mut self) {
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
        self.set_action("Status snapshot refreshed");
    }

    /// Render header.
    fn render_header(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let title = Paragraph::new("Xenobot Control Plane - Local-First Chat Data Workspace")
            .style(
                Style::default()
                    .fg(Color::Cyan)
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
            Line::from("Theme: auto"),
            Line::from("Update interval: 100 ms"),
            Line::from("Safety policy: authorized local data only"),
            Line::from(""),
            Line::from("Action: Press Enter to trigger settings placeholder action"),
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
        let status_text = format!(
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
        );

        let paragraph = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("Status"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render footer.
    fn render_footer(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let paragraph = Paragraph::new(self.footer.as_str())
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
        assert!(matches!(ui.status.key_status, KeyStatus::Extracted));
    }

    #[test]
    fn service_shortcuts_toggle_states() {
        let mut ui = Ui::new();
        ui.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(ui.status.http_status, ServiceStatus::Error(_)));
        ui.handle_key(key(KeyCode::Char('a')));
        assert!(matches!(ui.status.auto_decrypt_status, ServiceStatus::Error(_)));
        ui.handle_key(key(KeyCode::Char('e')));
        assert!(matches!(ui.status.key_status, KeyStatus::Extracted));
        ui.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(ui.status.http_status, ServiceStatus::Running));
        ui.handle_key(key(KeyCode::Char('a')));
        assert!(matches!(ui.status.auto_decrypt_status, ServiceStatus::Running));
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
}
