use app::App;
use std::time::Duration;

mod action;
mod app;
mod components;
mod config;
mod data;
mod theme;
mod tui;
mod utils;
mod widgets;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup panic handler
    std::panic::set_hook(Box::new(|panic_info| {
        use crossterm::{
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        // Restore terminal
        let _ = disable_raw_mode();
        let mut stderr = std::io::stderr();
        execute!(stderr, LeaveAlternateScreen).ok();

        // Print panic info
        eprintln!("{}", panic_info);
    }));

    // Create and run the app
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let mut app = App::new(Duration::from_millis(250)).await?;
        app.run().await
    })
}
