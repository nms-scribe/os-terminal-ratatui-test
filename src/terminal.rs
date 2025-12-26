use std::error::Error;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use keycode::{KeyMap, KeyMapping};
use os_terminal::font::TrueTypeFont;
use os_terminal::{ClipboardHandler, DrawTarget, MouseInput, Rgb, Terminal};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::scancode::PhysicalKeyExtScancode;
use winit::window::{ImePurpose, Window, WindowAttributes, WindowId};

use crate::tui::crossterm;
use std::io::{self, Write};
use ratatui::crossterm::event::Event;
use ratatui::prelude::{Backend, CrosstermBackend};
use terminput::Event as TermInputEvent;
use terminput_crossterm::to_crossterm;
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::backend::WindowSize;
use crate::tui::screen::Screen;

const DISPLAY_SIZE: (usize, usize) = (1024, 768);
const TOUCHPAD_SCROLL_MULTIPLIER: f32 = 0.25;

struct Clipboard(arboard::Clipboard);

impl Clipboard {
    fn new() -> Self {
        Self(arboard::Clipboard::new().unwrap())
    }
}

impl ClipboardHandler for Clipboard {
    fn get_text(&mut self) -> Option<String> {
        self.0.get_text().ok()
    }
    fn set_text(&mut self, text: String) {
        self.0.set_text(text).unwrap();
    }
}

struct TerminalWriter {
    terminal: Arc<Mutex<Terminal<Display>>>,
    pending_draw: Arc<AtomicBool>,
}

impl std::io::Write for TerminalWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut term) = self.terminal.lock() {
            term.process(buf);
            self.pending_draw.store(true, Ordering::Relaxed);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

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

struct GUIScreen {
    input_rx: Receiver<Event>,
    size_handle: Arc<Mutex<(u16, u16)>>
}


impl<W: Write> Screen<W> for GUIScreen {

    type Backend = VirtualBackend<W>;

    fn poll_and_read(&self, timeout: Duration) -> Result<Option<Event>,Box<dyn Error>> {
        match self.input_rx.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                Ok(None)
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // FUTURE: Should I be returning an error?
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected.into())
            }
        }
    }

    fn enable_raw_mode(&self) -> Result<(),Box<dyn Error>> {
        // nothing needs to be done here...
        Ok(())
    }

    fn disable_raw_mode(&self) -> Result<(),Box<dyn Error>> {
        // nothing needs to be done here...
        Ok(())
    }

    fn create_backend(&self, stdout: W) -> Self::Backend {
        let inner = CrosstermBackend::new(stdout);
        VirtualBackend::new(inner, self.size_handle.clone())
    }

    fn resize(&self, cols: u16, rows: u16) {
        *self.size_handle.lock().unwrap() = (cols, rows)
    }


}

fn run_tui_thread(writer: TerminalWriter, input_rx: Receiver<Event>, event_loop_proxy: EventLoopProxy<()>) {
    std::thread::spawn(move || {
        let screen = GUIScreen {
            input_rx,
            size_handle: Arc::new(Mutex::new((80, 24)))
        };
        let tick_rate = Duration::from_millis(250);
        if let Err(e) = crossterm::run(tick_rate, true, writer, screen) {
            eprintln!("TUI Error: {}", e);
        }
        // send event to signal that the thread is done...
        event_loop_proxy.send_event(())
    });
}

fn read_term_input(ansi: &str) -> Option<Event> {
    let event = TermInputEvent::parse_from(ansi.as_bytes()).unwrap();
    event.map(to_crossterm).transpose().unwrap()
}

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let display = Display::default();
    let buffer = display.buffer.clone();

    let (input_tx, input_rx) = channel::<Event>();

    let mut terminal = Terminal::new(display);
    terminal.set_auto_flush(false);
    terminal.set_scroll_speed(5);
    terminal.set_logger(|args| println!("Terminal Log: {:?}", args));
    terminal.set_clipboard(Box::new(Clipboard::new()));

    let input_tx_clone = input_tx.clone();
    terminal.set_pty_writer({
        Box::new(move |data| {
            if let Ok(s) = std::str::from_utf8(data.as_bytes()) {
                if let Some(event) = read_term_input(s) {
                    input_tx_clone.send(event).unwrap()
                }
            }
        })
    });

    let font_buffer = include_bytes!("FiraCodeNotoSans.ttf");
    terminal.set_font_manager(Box::new(TrueTypeFont::new(10.0, font_buffer)));
    terminal.set_history_size(1000);

    let terminal = Arc::new(Mutex::new(terminal));
    let pending_draw = Arc::new(AtomicBool::new(false));

    let event_loop = EventLoop::new()?;
    let event_loop_proxy = event_loop.create_proxy();

    let writer = TerminalWriter {
        terminal: terminal.clone(),
        pending_draw: pending_draw.clone(),
    };
    run_tui_thread(writer, input_rx, event_loop_proxy);

    let mut app = App::new(
        buffer.clone(),
        terminal.clone(),
        pending_draw.clone(),
        input_tx,
    );

    event_loop.run_app(&mut app)?;

    Ok(())
}

struct Display {
    width: usize,
    height: usize,
    buffer: Arc<Vec<AtomicU32>>,
}

impl Default for Display {
    fn default() -> Self {
        let buffer = (0..DISPLAY_SIZE.0 * DISPLAY_SIZE.1)
            .map(|_| AtomicU32::new(0))
            .collect::<Vec<_>>();

        Self {
            width: DISPLAY_SIZE.0,
            height: DISPLAY_SIZE.1,
            buffer: Arc::new(buffer),
        }
    }
}

impl DrawTarget for Display {
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    #[inline(always)]
    fn draw_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        let color = (color.0 as u32) << 16 | (color.1 as u32) << 8 | color.2 as u32;
        self.buffer[y * self.width + x].store(color, Ordering::Relaxed);
    }
}

struct App {
    buffer: Arc<Vec<AtomicU32>>,
    terminal: Arc<Mutex<Terminal<Display>>>,
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pending_draw: Arc<AtomicBool>,
    input_tx: Sender<Event>,
    scroll_accumulator: f32,
}

impl App {
    fn new(
        buffer: Arc<Vec<AtomicU32>>,
        terminal: Arc<Mutex<Terminal<Display>>>,
        pending_draw: Arc<AtomicBool>,
        input_tx: Sender<Event>,
    ) -> Self {
        Self {
            buffer,
            terminal,
            window: None,
            surface: None,
            pending_draw,
            input_tx,
            scroll_accumulator: 0.0,
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if !matches!(cause, StartCause::ResumeTimeReached { .. })
            || !self.pending_draw.swap(false, Ordering::Relaxed)
        {
            return;
        }
        if let Some(surface) = self.surface.as_mut() {
            self.terminal.lock().unwrap().flush();

            let mut buffer = surface.buffer_mut().unwrap();
            for (index, value) in self.buffer.iter().enumerate() {
                buffer[index] = value.load(Ordering::Relaxed);
            }

            buffer.present().unwrap();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let refresh_rate = event_loop
            .primary_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .unwrap_or(60000);

        let frame_duration = 1000.0 / (refresh_rate as f32 / 1000.0);
        let duration = Duration::from_millis(frame_duration as u64);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + duration));
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (width, height) = DISPLAY_SIZE;
        let attributes = WindowAttributes::default()
            .with_title("Terminal")
            .with_resizable(false)
            .with_inner_size(PhysicalSize::new(width as f64, height as f64));

        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        window.set_ime_allowed(true);
        window.set_ime_purpose(ImePurpose::Terminal);

        let context = Context::new(window.clone()).unwrap();
        let mut surface = Surface::new(&context, window.clone()).unwrap();

        surface
            .resize(
                NonZeroU32::new(width as u32).unwrap(),
                NonZeroU32::new(height as u32).unwrap(),
            )
            .unwrap();

        self.window = Some(window);
        self.surface = Some(surface);

        let terminal = self.terminal.lock().unwrap();
        let (cols, rows) = (terminal.columns(), terminal.rows());
        self.input_tx
            .send(Event::Resize(cols as u16, rows as u16))
            .unwrap();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: ()) {
        // if I receive this then the terminal loop is done...
        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if let Some(event) = read_term_input(&text) {
                    self.input_tx.send(event).unwrap();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll_accumulator += match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines,
                    MouseScrollDelta::PixelDelta(delta) => {
                        delta.y as f32 * TOUCHPAD_SCROLL_MULTIPLIER
                    }
                };
                if self.scroll_accumulator.abs() >= 1.0 {
                    let lines = self.scroll_accumulator as isize;
                    self.scroll_accumulator -= lines as f32;
                    self.terminal
                        .lock()
                        .unwrap()
                        .handle_mouse(MouseInput::Scroll(lines));
                    self.pending_draw.store(true, Ordering::Relaxed);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(evdev_code) = event.physical_key.to_scancode() {
                    if let Ok(keymap) =
                        // FUTURE: from os-terminal author: "Note: remember to change KeyMapping::Evdev to something else if you run on other platforms like Windows."
                        KeyMap::from_key_mapping(KeyMapping::Evdev(evdev_code as u16))
                    {
                        // Windows scancode is 16-bit extended scancode
                        let mut scancode = keymap.win;
                        if event.state == ElementState::Released {
                            scancode += 0x80;
                        }
                        if scancode >= 0xe000 {
                            self.terminal.lock().unwrap().handle_keyboard(0xe0);
                            scancode -= 0xe000;
                        }
                        self.terminal
                            .lock()
                            .unwrap()
                            .handle_keyboard(scancode as u8);
                        self.pending_draw.store(true, Ordering::Relaxed);
                    }
                }
            }
            _ => {}
        }
    }
}
