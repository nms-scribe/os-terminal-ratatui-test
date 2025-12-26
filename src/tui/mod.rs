use std::error::Error;
use std::time::Duration;
use crate::tui::crossterm::CrosstermScreen;

pub(crate) mod screen;
mod app;
pub(crate) mod crossterm;
mod ui;

pub(crate) fn run_no_win() -> Result<(), Box<dyn Error>> {
    let tick_rate = Duration::from_millis(250);
    crossterm::run(tick_rate, true, std::io::stdout(), CrosstermScreen)?;
    Ok(())
}
