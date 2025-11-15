//! Interactive TUI dashboard for monitoring and testing hedged RPC performance.
//!
//! This binary provides a real-time dashboard for testing Solana RPC providers
//! with hedged request strategies. Features include:
//! - Live performance metrics and charts
//! - Multiple provider racing modes
//! - Batch testing capabilities
//! - Per-provider statistics and latency trends

mod app;
mod env;
mod rpc;
mod ui;

use std::time::Duration;

use app::{App, AppEvent};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use env::build_client_from_env;
use rpc::spawn_rpc_call;
use tokio::sync::mpsc;
use ui::draw_ui;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let (client, providers_cfg) = build_client_from_env()?;
    let mut app = App::new(client, providers_cfg)?;

    let mut terminal = ratatui::init();
    terminal.clear()?;

    let result = run_app(&mut terminal, &mut app).await;

    ratatui::restore();

    result
}

async fn run_app(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    app.refresh_stats();

    loop {
        while let Ok(ev) = rx.try_recv() {
            match ev {
                AppEvent::RpcFinished {
                    provider,
                    latency_ms,
                    ok,
                    message,
                } => {
                    app.set_last_result(provider, latency_ms, ok, message);
                }
            }
        }

        terminal.draw(|frame| draw_ui(frame, app))?;

        if app.should_run_call() {
            spawn_rpc_call(app, tx.clone());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) = event::read()?
            {
                match code {
                    KeyCode::Char('q') => break,
                    KeyCode::Up => app.prev_provider(),
                    KeyCode::Down => app.next_provider(),
                    KeyCode::Tab => app.toggle_mode(),
                    KeyCode::Char('m') => app.toggle_method(),
                    KeyCode::Char('r') => {
                        spawn_rpc_call(app, tx.clone());
                    }
                    KeyCode::Char(' ') => {
                        app.mode = app::Mode::SingleProvider;
                        spawn_rpc_call(app, tx.clone());
                    }
                    KeyCode::Char('b') => {
                        app.toggle_batch_mode();
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        app.increase_provider_count();
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        app.decrease_provider_count();
                    }
                    KeyCode::Char('[') | KeyCode::Char(',') => {
                        app.decrease_batch_count();
                    }
                    KeyCode::Char(']') | KeyCode::Char('.') | KeyCode::Char('/') => {
                        app.increase_batch_count();
                    }
                    KeyCode::Char('s') => {
                        app.stats_snapshot.clear();
                        app.last_message = "Stats reset".to_string();
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
