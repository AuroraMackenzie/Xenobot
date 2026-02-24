//! Event handling for TUI.
//!
//! Manages terminal events (keyboard, mouse, resize) with a tick-based
//! system for real-time updates.

use crate::error::{Result, TuiError};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, Instant};

/// TUI events.
#[derive(Debug, Clone)]
pub enum Event {
    /// Tick event for periodic updates.
    Tick,
    /// Key press.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Terminal resize.
    Resize(u16, u16),
    /// Terminal gained focus.
    FocusGained,
    /// Terminal lost focus.
    FocusLost,
    /// Pasted text.
    Paste(String),
}

/// Event handler.
pub struct EventHandler {
    /// Event receiver.
    receiver: mpsc::Receiver<Event>,
    /// Event sender.
    sender: mpsc::Sender<Event>,
    /// Event handler thread handle.
    handler: Option<thread::JoinHandle<()>>,
    /// Shutdown signal.
    shutdown: Arc<AtomicBool>,
    /// Tick interval.
    tick_rate: Duration,
}

impl EventHandler {
    /// Create a new event handler.
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::channel();
        let handler = None; // Will be set in start()
        let shutdown = Arc::new(AtomicBool::new(false));
        Self {
            receiver,
            sender,
            handler,
            shutdown,
            tick_rate,
        }
    }

    /// Start the event handler thread.
    pub fn start(&mut self) -> Result<()> {
        let tick_rate = self.tick_rate;
        let sender = self.sender.clone();
        let shutdown = Arc::clone(&self.shutdown);

        let handler = thread::spawn(move || {
            let mut last_tick = Instant::now();

            while !shutdown.load(Ordering::Relaxed) {
                // Calculate timeout for next tick
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);

                // Poll for events with timeout
                if event::poll(timeout)
                    .map_err(|e| {
                        tracing::error!(%e, "Event poll error");
                    })
                    .unwrap_or(false)
                {
                    // Event available
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if key.kind == event::KeyEventKind::Press {
                                let _ = sender.send(Event::Key(key));
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            let _ = sender.send(Event::Mouse(mouse));
                        }
                        Ok(CrosstermEvent::Resize(width, height)) => {
                            let _ = sender.send(Event::Resize(width, height));
                        }
                        Ok(CrosstermEvent::FocusGained) => {
                            let _ = sender.send(Event::FocusGained);
                        }
                        Ok(CrosstermEvent::FocusLost) => {
                            let _ = sender.send(Event::FocusLost);
                        }
                        Ok(CrosstermEvent::Paste(data)) => {
                            let _ = sender.send(Event::Paste(data));
                        }
                        Err(e) => {
                            tracing::error!(%e, "Event read error");
                        }
                    }
                }

                // Send tick if enough time has elapsed
                if last_tick.elapsed() >= tick_rate {
                    let _ = sender.send(Event::Tick);
                    last_tick = Instant::now();
                }
            }
        });

        self.handler = Some(handler);
        Ok(())
    }

    /// Get the next event.
    pub fn next(&self) -> Result<Event> {
        self.receiver
            .recv()
            .map_err(|e| TuiError::Event(e.to_string()))
    }

    /// Try to get an event without blocking.
    pub fn try_next(&self) -> Option<Event> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for EventHandler {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);

        // Wait for handler thread to finish
        if let Some(handler) = self.handler.take() {
            let _ = handler.join();
        }
    }
}
