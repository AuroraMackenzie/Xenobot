//! Terminal user interface for Xenobot.
//!
//! This crate provides a TUI for real-time monitoring, data exploration,
//! and interactive analysis of chat data.

#![deny(missing_docs, unsafe_code)]

/// TUI application state and lifecycle.
pub mod app;

/// UI components and widgets.
pub mod ui;

/// Event handling and input processing.
pub mod events;

/// Error types for TUI operations.
pub mod error;

// Re-exports
pub use app::App;
pub use error::{Result, TuiError};
pub use events::{Event, EventHandler};
pub use ui::{Ui, UiComponent};

/// TUI configuration.
#[derive(Debug, Clone)]
pub struct TuiConfig {
    /// Whether TUI is enabled.
    pub enabled: bool,
    /// Update interval in milliseconds.
    pub update_interval_ms: u64,
    /// Maximum FPS for rendering.
    pub max_fps: u32,
    /// Whether to enable mouse support.
    pub mouse_enabled: bool,
    /// Terminal theme (light/dark/auto).
    pub theme: String,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            update_interval_ms: 1000, // 1 second
            max_fps: 30,
            mouse_enabled: false,
            theme: "auto".to_string(),
        }
    }
}

/// Initialize the TUI application.
///
/// # Errors
/// Returns an error if terminal initialization fails.
pub fn init() -> Result<App> {
    App::new()
}

/// Run the TUI application.
///
/// # Errors
/// Returns an error if the application loop fails.
pub fn run() -> Result<()> {
    let mut app = init()?;
    app.run()
}
