mod app;
mod ui;

mod eventbrite_attendees;
mod eventbrite_auth;

use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{debug, error};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, time::Duration, time::Instant};
use ui::draw_ui;
use simplelog::*;
use std::fs::File;
use std::path::PathBuf;

fn main() -> Result<(), io::Error> {
    // Initialize logging
    let log_file_path = get_log_file_path();
    CombinedLogger::init(vec![
        WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            File::create(log_file_path).unwrap(),
        ),
    ])
    .unwrap();

    debug!("Application started");

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        error!("Application error: {:?}", err);
        println!("Error: {:?}", err);
    }

    debug!("Application exited");
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    debug!("Entering application loop");
    let mut last_event_time = Instant::now();
    let debounce_duration = Duration::from_millis(150);

    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if last_event_time.elapsed() >= debounce_duration {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        debug!("Ignoring non-press key event: {:?}", key);
                        continue;
                    }

                    debug!("Key event received: {:?}", key);
                    last_event_time = Instant::now();
                    if let Some(action) = app.handle_input(key.code) {
                        debug!("Action triggered: {}", action);
                        if action == "quit" {
                            debug!("Quit action received, exiting loop");
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

fn get_log_file_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Unable to find home directory");
    home_dir.join(".les_ptits_gilets.log")
}