use std::{
    error::Error,
    io::Write,
    time::Duration,
};

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
};

use crate::tui::{app::App, screen::Screen};

pub(crate) struct CrosstermScreen;

impl<W: Write> Screen<W> for CrosstermScreen {

    type Backend = CrosstermBackend<W>;

    fn poll_and_read(&self, timeout: Duration) -> Result<Option<Event>,Box<dyn Error>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    fn enable_raw_mode(&self) -> Result<(),Box<dyn Error>> {
        Ok(enable_raw_mode()?)
    }

    fn disable_raw_mode(&self) -> Result<(),Box<dyn Error>> {
        Ok(disable_raw_mode()?)
    }

    fn create_backend(&self, stdout: W) -> Self::Backend {
        CrosstermBackend::new(stdout)
    }

    fn resize(&self, _cols: u16, _rows: u16) {
        // do nothing, the terminal can't resize
    }



}


pub fn run<W: Write, S: Screen<W>>(
    tick_rate: Duration,
    enhanced_graphics: bool,
    mut stdout: W,
    screen: S,
) -> Result<(), Box<dyn Error>> {
    // setup terminal
    screen.enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = screen.create_backend(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new("Crossterm Demo", enhanced_graphics);
    let app_result = app.run(&mut terminal, tick_rate, &screen);

    // restore terminal
    screen.disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = app_result {
        println!("{err:?}");
    }

    Ok(())
}
