use std::{error::Error, time::Duration};

mod app;
mod crossterm;
mod ui;


pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let tick_rate = Duration::from_millis(250);
    crossterm::run(tick_rate, true, std::io::stdout())?;
    Ok(())
}
