//! UI components for Xenobot TUI.
//!
//! Implements the terminal user interface with menus, status bars, forms,
//! and real-time status display.

use crate::error::Result;
use crossterm::event::KeyEvent;
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
}

/// Available tabs.
#[allow(dead_code)]
enum Tab {
    MainMenu,
    KeyExtraction,
    Decryption,
    Services,
    Settings,
    Accounts,
}

/// Menu state.
struct MenuState {
    /// Current menu selection.
    selection: usize,
    /// Menu items.
    items: Vec<String>,
}

/// Status information.
struct Status {
    /// Account information.
    account: String,
    /// WeChat PID.
    pid: Option<u32>,
    /// WeChat version.
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
#[allow(dead_code)]
enum KeyStatus {
    NotExtracted,
    Extracted,
    Error(String),
}

/// Service status.
#[allow(dead_code)]
enum ServiceStatus {
    Stopped,
    Running,
    Error(String),
}

impl Ui {
    /// Create a new UI.
    pub fn new() -> Self {
        Self {
            active_tab: Tab::MainMenu,
            menu_state: MenuState {
                selection: 0,
                items: vec![
                    "Get Keys".to_string(),
                    "Decrypt Data".to_string(),
                    "Start/Stop HTTP Service".to_string(),
                    "Start/Stop Auto Decryption".to_string(),
                    "Settings".to_string(),
                    "Switch Account".to_string(),
                    "Exit".to_string(),
                ],
            },
            status: Status {
                account: "Not logged in".to_string(),
                pid: None,
                version: "Unknown".to_string(),
                key_status: KeyStatus::NotExtracted,
                data_size: "0 B".to_string(),
                http_status: ServiceStatus::Stopped,
                auto_decrypt_status: ServiceStatus::Stopped,
                last_session_time: "Never".to_string(),
            },
            footer: "Xenobot TUI - Press 'q' to quit, arrow keys to navigate".to_string(),
            width: 0,
            height: 0,
        }
    }

    /// Render the entire UI.
    pub fn render(&mut self, frame: &mut Frame) -> Result<()> {
        let size = frame.size();
        self.width = size.width;
        self.height = size.height;

        // Split layout
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

        // Render header
        self.render_header(frame, chunks[0])?;

        // Render tabs
        self.render_tabs(frame, chunks[1])?;

        // Render main content based on active tab
        match self.active_tab {
            Tab::MainMenu => self.render_main_menu(frame, chunks[2])?,
            Tab::KeyExtraction => self.render_key_extraction(frame, chunks[2])?,
            Tab::Decryption => self.render_decryption(frame, chunks[2])?,
            Tab::Services => self.render_services(frame, chunks[2])?,
            Tab::Settings => self.render_settings(frame, chunks[2])?,
            Tab::Accounts => self.render_accounts(frame, chunks[2])?,
        }

        // Render status bar
        self.render_status_bar(frame, chunks[3])?;

        // Render footer
        self.render_footer(frame, chunks[4])?;

        Ok(())
    }

    /// Handle key event.
    pub fn handle_key(&mut self, _key: KeyEvent) {
        // TODO: Implement key handling
    }

    /// Update UI state.
    pub fn update(&mut self) {
        // TODO: Update status from backend
    }

    /// Resize UI.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Render header.
    fn render_header(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let title = Paragraph::new("Xenobot - WeChat Data Extraction & Analysis")
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
        let tabs = Tabs::new(vec![
            "Main Menu",
            "Key Extraction",
            "Decryption",
            "Services",
            "Settings",
            "Accounts",
        ])
        .select(match self.active_tab {
            Tab::MainMenu => 0,
            Tab::KeyExtraction => 1,
            Tab::Decryption => 2,
            Tab::Services => 3,
            Tab::Settings => 4,
            Tab::Accounts => 5,
        })
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
                ListItem::new(Span::styled(item.clone(), style))
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
        let text = vec![Line::from("Key Extraction (placeholder)")];
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
        let text = vec![Line::from("Decryption (placeholder)")];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Decryption"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render services tab.
    fn render_services(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![Line::from("Services (placeholder)")];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Services"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render settings tab.
    fn render_settings(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![Line::from("Settings (placeholder)")];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Settings"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render accounts tab.
    fn render_accounts(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let text = vec![Line::from("Accounts (placeholder)")];
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Accounts"));
        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render status bar.
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let status_text = format!(
            "Account: {} | PID: {} | Version: {} | Key: {} | Data: {} | HTTP: {} | Auto-Decrypt: {} | Last Session: {}",
            self.status.account,
            self.status.pid.map_or("N/A".to_string(), |pid| pid.to_string()),
            self.status.version,
            match &self.status.key_status {
                KeyStatus::NotExtracted => "Not extracted".to_string(),
                KeyStatus::Extracted => "Extracted".to_string(),
                KeyStatus::Error(e) => format!("Error: {}", e),
            },
            self.status.data_size,
            match &self.status.http_status {
                ServiceStatus::Stopped => "Stopped".to_string(),
                ServiceStatus::Running => "Running".to_string(),
                ServiceStatus::Error(e) => format!("Error: {}", e),
            },
            match &self.status.auto_decrypt_status {
                ServiceStatus::Stopped => "Stopped".to_string(),
                ServiceStatus::Running => "Running".to_string(),
                ServiceStatus::Error(e) => format!("Error: {}", e),
            },
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
}
