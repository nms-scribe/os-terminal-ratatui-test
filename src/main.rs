use std::error::Error;

mod tui;
mod terminal;

fn main() -> Result<(), Box<dyn Error>> {
    if let Some("--no-win") = std::env::args().skip(1).next().as_deref() {
        tui::run_no_win()
    } else {
        terminal::run()
    }
}
