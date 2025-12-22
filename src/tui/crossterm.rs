use std::{
    error::Error,
    io::{self, Write},
    sync::{Arc, Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen},
    },
};

use crate::tui::{AppEvent, VirtualBackend, app::App, ui};

pub fn run<W: Write>(
    tick_rate: Duration,
    enhanced_graphics: bool,
    mut stdout: W,
    input_rx: Receiver<AppEvent>,
    size_handle: Arc<Mutex<(u16, u16)>>,
) -> Result<(), Box<dyn Error>> {
    // setup terminal
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let inner = CrosstermBackend::new(stdout);
    let backend = VirtualBackend::new(inner, size_handle.clone());
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new("Crossterm Demo", enhanced_graphics);
    let app_result = run_app(&mut terminal, app, tick_rate, input_rx, size_handle);

    // restore terminal
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

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
    input_rx: Receiver<AppEvent>,
    size_handle: Arc<Mutex<(u16, u16)>>,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        match input_rx.recv_timeout(timeout) {
            Ok(AppEvent::Input(ansi)) => {
                let key_code = match ansi.as_str() {
                    "\x1b[A" => Some(KeyCode::Up),
                    "\x1b[B" => Some(KeyCode::Down),
                    "\x1b[C" => Some(KeyCode::Right),
                    "\x1b[D" => Some(KeyCode::Left),
                    s if s.len() == 1 => Some(KeyCode::Char(s.chars().next().unwrap())),
                    _ => None,
                };

                if let Some(code) = key_code {
                    match code {
                        KeyCode::Left | KeyCode::Char('h') => app.on_left(),
                        KeyCode::Up | KeyCode::Char('k') => app.on_up(),
                        KeyCode::Right | KeyCode::Char('l') => app.on_right(),
                        KeyCode::Down | KeyCode::Char('j') => app.on_down(),
                        KeyCode::Char(c) => app.on_key(c),
                        _ => {}
                    }
                }
            }
            Ok(AppEvent::Resize(cols, rows)) => {
                *size_handle.lock().unwrap() = (cols, rows);
            }
            Err(_) => {}
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
