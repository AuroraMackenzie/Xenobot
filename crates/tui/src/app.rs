//! Main TUI application for Xenobot.
//!
//! Provides terminal user interface for managing WeChat data extraction,
//! decryption, monitoring, and API services.

use crate::error::{Result, TuiError};
use crate::events::{Event, EventHandler};
use crate::ui::Ui;
use crossterm::{
    event::{self, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

/// Main TUI application.
pub struct App {
    /// Terminal instance.
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    /// Event handler.
    event_handler: EventHandler,
    /// UI component manager.
    ui: Ui,
    /// Application running state.
    running: bool,
}

impl App {
    /// Create a new TUI application.
    pub fn new() -> Result<Self> {
        // Setup terminal
        enable_raw_mode().map_err(|e| TuiError::TerminalInit(e.to_string()))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)
            .map_err(|e| TuiError::TerminalInit(e.to_string()))?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).map_err(|e| TuiError::TerminalInit(e.to_string()))?;

        // Create event handler with 100ms tick rate
        let mut event_handler = EventHandler::new(Duration::from_millis(100));
        event_handler.start()?;

        // Create UI
        let ui = Ui::new();

        Ok(Self {
            terminal,
            event_handler,
            ui,
            running: true,
        })
    }

    /// Run the main application loop.
    pub fn run(&mut self) -> Result<()> {
        while self.running {
            // Render UI
            self.terminal
                .draw(|frame| {
                    if let Err(e) = self.ui.render(frame) {
                        // Log error but continue
                        tracing::error!(%e, "UI rendering error");
                    }
                })
                .map_err(|e| TuiError::Render(e.to_string()))?;

            // Handle events
            match self.event_handler.next()? {
                Event::Tick => {
                    // Update UI state
                    self.ui.update();
                }
                Event::Key(key) => {
                    self.handle_key(key)?;
                }
                Event::Resize(width, height) => {
                    self.ui.resize(width, height);
                }
                Event::Mouse(_) => {
                    // Mouse events not currently used
                }
                Event::FocusGained => {}
                Event::FocusLost => {}
                Event::Paste(_) => {}
            }
        }
        Ok(())
    }

    /// Handle key events.
    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            event::KeyCode::Char('q') => {
                self.running = false;
            }
            event::KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {
                // Delegate to UI components
                self.ui.handle_key(key);
            }
        }
        Ok(())
    }

    /// Shutdown the application and restore terminal.
    pub fn shutdown(&mut self) -> Result<()> {
        // Restore terminal
        disable_raw_mode().map_err(|e| TuiError::TerminalInit(e.to_string()))?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)
            .map_err(|e| TuiError::TerminalInit(e.to_string()))?;
        self.terminal
            .show_cursor()
            .map_err(|e| TuiError::TerminalInit(e.to_string()))?;
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
