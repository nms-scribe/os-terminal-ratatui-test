use std::io::Write;
use ratatui::prelude::Backend;
use std::time::Duration;
use ratatui::crossterm::event::Event;
use std::error::Error;


pub trait Screen<W: Write> {

    type Backend: Backend + std::io::Write;

    fn poll_and_read(&self, timeout: Duration) -> Result<Option<Event>,Box<dyn Error>>;

    fn enable_raw_mode(&self) -> Result<(),Box<dyn Error>>;

    fn disable_raw_mode(&self) -> Result<(),Box<dyn Error>>;

    fn resize(&self, cols: u16, rows: u16);

    fn create_backend(&self, stdout: W) -> Self::Backend;

}
