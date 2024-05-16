use std::{path::PathBuf, time::Instant};

use eyre::Result;

mod app;
mod tui;
use app::PlayerApp;
use tui::Tui;

fn main() -> Result<()> {
    let args = std::env::args();
    if args.len() != 2 {
        return Err(eyre::eyre!("Must provide a path to search for files."));
    }
    let root_dir = PathBuf::from(&args.collect::<Vec<_>>()[1]);

    let mut tui = Tui::new()?;
    let mut app = PlayerApp::new(&root_dir)?;
    let mut dt = 0.0;

    while app.is_alive() {
        let start = Instant::now();
        app.update(dt)?;
        tui.update(&mut app)?;
        dt = start.elapsed().as_secs_f64();
    }

    Ok(())
}
