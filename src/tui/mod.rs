use ratatui::backend::{Backend, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::prelude::*;
use std::error::Error;
use std::io;
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod app;
mod crossterm;
mod ui;

pub struct VirtualBackend<W: io::Write> {
    inner: CrosstermBackend<W>,
    size: Arc<Mutex<(u16, u16)>>,
}

impl<W: io::Write> VirtualBackend<W> {
    pub fn new(inner: CrosstermBackend<W>, size: Arc<Mutex<(u16, u16)>>) -> Self {
        Self { inner, size }
    }
}

impl<W: io::Write> io::Write for VirtualBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

impl<W: io::Write> Backend for VirtualBackend<W> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<ratatui::layout::Position>>(
        &mut self,
        position: P,
    ) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn size(&self) -> io::Result<Size> {
        let (w, h) = *self.size.lock().unwrap();
        Ok(Size::new(w, h))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let (w, h) = *self.size.lock().unwrap();
        Ok(WindowSize {
            columns_rows: Size {
                width: w,
                height: h,
            },
            pixels: Size {
                width: 0,
                height: 0,
            },
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }

    fn append_lines(&mut self, _n: u16) -> io::Result<()> {
        Ok(())
    }

    fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        let ratatui::prelude::Position { x, y } = self.get_cursor_position()?;
        Ok((x, y))
    }

    fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        self.set_cursor_position(ratatui::prelude::Position { x, y })
    }
}

pub enum AppEvent {
    Input(String),
    Resize(u16, u16),
}

pub fn run<W: Write + Send + 'static>(
    writer: W,
    input_rx: Receiver<AppEvent>,
) -> Result<(), Box<dyn Error>> {
    let tick_rate = Duration::from_millis(250);
    let shared_size = Arc::new(Mutex::new((80, 24)));
    crossterm::run(tick_rate, true, writer, input_rx, shared_size)?;
    Ok(())
}
