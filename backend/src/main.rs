mod app;
mod config;
mod docker;
mod error;
mod ports;
mod process;
mod server;
mod state;
mod terminal;
mod tui;
mod worktree;

use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use error::Result;
use state::ensure_data_dir;
use tui::Panel;

#[derive(Parser)]
#[command(name = "envibe")]
#[command(about = "Multi-project development environment manager")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run in server mode (for Electron/web frontend)
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "3847")]
        port: u16,
    },
    /// Run in TUI mode (default)
    Tui,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let data_dir = ensure_data_dir().await?;

    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Server { port } => {
            // Server mode - logging to stdout
            tracing_subscriber::fmt()
                .with_env_filter("envibe=debug,tower_http=debug")
                .init();

            server::run_server(data_dir, port).await?;
        }
        Commands::Tui => {
            // TUI mode - logging to file
            let log_file = std::fs::File::create(data_dir.join("envibe.log"))?;
            tracing_subscriber::fmt()
                .with_writer(log_file)
                .with_env_filter("envibe=debug")
                .init();

            run_tui(data_dir).await?;
        }
    }

    Ok(())
}

async fn run_tui(data_dir: std::path::PathBuf) -> Result<()> {
    // Create application
    let mut app = App::new(data_dir).await?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Cleanup
    app.cleanup().await?;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);

    loop {
        // Process any pending log messages
        app.process_logs();

        // Draw UI
        terminal.draw(|f| tui::ui::render(f, app))?;

        // Check for events with timeout
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                // Handle global keys
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    (KeyCode::Char('?'), _) => {
                        app.show_help = !app.show_help;
                    }
                    (KeyCode::Esc, _) => {
                        if app.show_help {
                            app.show_help = false;
                        }
                    }
                    (KeyCode::Tab, _) => {
                        if !app.show_help {
                            app.focused_panel = app.focused_panel.next();
                        }
                    }
                    (KeyCode::BackTab, _) => {
                        if !app.show_help {
                            app.focused_panel = app.focused_panel.prev();
                        }
                    }
                    _ => {
                        if !app.show_help {
                            handle_panel_input(app, key.code).await?;
                        }
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_panel_input(app: &mut App, key: KeyCode) -> Result<()> {
    match app.focused_panel {
        Panel::Projects => handle_projects_input(app, key).await,
        Panel::Services => handle_services_input(app, key).await,
        Panel::Console => handle_console_input(app, key),
    }
}

async fn handle_projects_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            app.navigate_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.navigate_up();
        }
        KeyCode::Enter => {
            app.select_project();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_services_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            app.navigate_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.navigate_up();
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Err(e) = app.toggle_service().await {
                app.log(format!("Error: {}", e));
            }
        }
        KeyCode::Char('r') => {
            if let Err(e) = app.restart_service().await {
                app.log(format!("Error: {}", e));
            }
        }
        KeyCode::Char('a') => {
            if let Err(e) = app.start_all_services().await {
                app.log(format!("Error: {}", e));
            }
        }
        KeyCode::Char('s') => {
            if let Err(e) = app.stop_all_services().await {
                app.log(format!("Error: {}", e));
            }
        }
        KeyCode::Char('l') => {
            app.focused_panel = Panel::Console;
        }
        _ => {}
    }
    Ok(())
}

fn handle_console_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            app.navigate_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.navigate_up();
        }
        KeyCode::PageDown => {
            app.page_down();
        }
        KeyCode::PageUp => {
            app.page_up();
        }
        KeyCode::Char('f') => {
            app.toggle_follow();
        }
        KeyCode::Char('c') => {
            app.clear_logs();
        }
        _ => {}
    }
    Ok(())
}
