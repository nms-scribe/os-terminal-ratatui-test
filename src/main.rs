use std::error::Error;



mod tui;
mod terminal;

fn main() -> Result<(), Box<dyn Error>> {
    terminal::run()
}
