use crate::action::Action;
use ratatui::Frame;

pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod process;

/// The Component trait defines the interface that all UI components must implement
#[allow(dead_code)]
pub trait Component {
    /// Handle events for the component
    fn handle_events(
        &mut self,
        event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>>;

    /// Update the component state based on actions
    fn update(&mut self, action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>>;

    /// Render the component to the terminal frame
    fn render(&mut self, f: &mut Frame);
}
