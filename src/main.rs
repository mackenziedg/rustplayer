use std::time::Instant;

use eyre::Result;

mod app;
mod tui;
use app::PlayerApp;
use tui::Tui;

fn main() -> Result<()> {
    let mut tui = Tui::new()?;
    let mut app = PlayerApp::new()?;
    let mut dt = 0.0;

    while app.is_alive() {
        let start = Instant::now();
        app.update(dt)?;
        tui.update(&mut app)?;
        dt = start.elapsed().as_secs_f64();
    }

    Ok(())
}
