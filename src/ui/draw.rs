//! UI rendering functions for the TUI dashboard.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
};

use super::styles::*;
use crate::app::{App, Method, Mode};

pub fn draw_ui(frame: &mut Frame, app: &App) {
    let size = frame.area();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(0),
            Constraint::Length(7),
        ])
        .split(size);

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main_layout[2]);

    draw_header(frame, main_layout[0]);
    draw_session_stats(frame, main_layout[1], app);
    draw_providers_table(frame, body_layout[0], app);
    draw_detail_panel(frame, body_layout[1], app);
    draw_keybinds(frame, main_layout[3]);
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let title = " Hedged RPC Client :: Real-time Dashboard ";
    let block = Block::default()
        .title(title)
        .title_style(header_style())
        .borders(Borders::ALL)
        .border_style(border_style());

    frame.render_widget(block, area);
}

fn draw_session_stats(frame: &mut Frame, area: Rect, app: &App) {
    let uptime = app.session_uptime();
    let uptime_str = if uptime.as_secs() < 60 {
        format!("{}s", uptime.as_secs())
    } else if uptime.as_secs() < 3600 {
        format!("{}m {}s", uptime.as_secs() / 60, uptime.as_secs() % 60)
    } else {
        format!(
            "{}h {}m",
            uptime.as_secs() / 3600,
            (uptime.as_secs() % 3600) / 60
        )
    };

    let success_rate = app.success_rate();
    let calls_per_sec = app.calls_per_second();
    let avg_latency = app.average_latency();

    let success_bar_width: usize = 15;
    let success_filled = ((success_rate / 100.0) * success_bar_width as f64) as usize;
    let success_bar = format!(
        "[{}{}]",
        "█".repeat(success_filled),
        "░".repeat(success_bar_width.saturating_sub(success_filled))
    );

    let text = vec![
        Line::from(vec![
            Span::raw("Session: ").style(muted_style()),
            Span::raw(format!("Uptime: {} ", uptime_str)).style(Style::default().fg(TEXT_COLOR)),
            Span::raw("│ ").style(muted_style()),
            Span::raw(format!("Total Calls: {} ", app.total_calls))
                .style(Style::default().fg(TEXT_COLOR)),
            Span::raw("│ ").style(muted_style()),
            Span::raw(format!("Success: {} ", app.total_successes)).style(success_style()),
            Span::raw("│ ").style(muted_style()),
            Span::raw(format!("Errors: {}", app.total_errors)).style(if app.total_errors > 0 {
                error_style()
            } else {
                Style::default().fg(TEXT_COLOR)
            }),
        ]),
        Line::from(vec![
            Span::raw("Performance: ").style(muted_style()),
            Span::raw(format!("{:.1} calls/s ", calls_per_sec)).style(highlight_style()),
            Span::raw("│ ").style(muted_style()),
            Span::raw(format!("Avg Latency: {:.0}ms ", avg_latency))
                .style(Style::default().fg(TEXT_COLOR)),
            Span::raw("│ ").style(muted_style()),
            Span::raw("Success Rate: ").style(muted_style()),
            Span::raw(success_bar).style(if success_rate > 95.0 {
                success_style()
            } else if success_rate > 80.0 {
                highlight_style()
            } else {
                error_style()
            }),
            Span::raw(format!(" {:.1}%", success_rate)).style(Style::default().fg(TEXT_COLOR)),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Session Analytics ")
            .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(border_style()),
    );

    frame.render_widget(paragraph, area);
}

fn draw_providers_table(frame: &mut Frame, area: Rect, app: &App) {
    let stats = &app.stats_snapshot;

    let header_cells = [
        "Provider",
        "Wins",
        "Avg ms",
        "Errors",
        "Latency Trend",
        "Win Rate",
    ]
    .into_iter()
    .map(|h| Cell::from(h).style(table_header_style()));

    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let total_wins: u64 = stats.values().map(|s| s.wins).sum();

    let rows = app.providers.iter().enumerate().map(|(idx, (id, _url))| {
        let snapshot = stats.get(id);
        let wins = snapshot.map(|s| s.wins).unwrap_or(0);
        let avg_ms = snapshot.map(|s| s.avg_latency_ms).unwrap_or(0.0);
        let errors = snapshot.map(|s| s.errors).unwrap_or(0);

        let win_rate = if total_wins > 0 {
            wins as f64 / total_wins as f64 * 100.0
        } else {
            0.0
        };

        let history = app.latency_history.get(id).cloned().unwrap_or_default();
        let sparkline_data: Vec<u64> = history.iter().copied().collect();
        let sparkline_str = if sparkline_data.is_empty() {
            "───────────".to_string()
        } else {
            create_mini_sparkline(&sparkline_data)
        };

        let bar_width: usize = 10;
        let filled = ((win_rate / 100.0) * bar_width as f64) as usize;
        let win_bar = format!(
            "[{}{}] {:.0}%",
            "█".repeat(filled),
            "░".repeat(bar_width.saturating_sub(filled)),
            win_rate
        );

        let win_style = if wins > 0 {
            success_style()
        } else {
            Style::default()
        };
        let error_style_cell = if errors > 0 {
            error_style()
        } else {
            Style::default()
        };

        let sparkline_style = if avg_ms < 300.0 {
            success_style()
        } else if avg_ms < 600.0 {
            highlight_style()
        } else {
            error_style()
        };

        let cells = vec![
            Cell::from(id.0.to_string()),
            Cell::from(format!("{}", wins)).style(win_style),
            Cell::from(format!("{:.1}", avg_ms)),
            Cell::from(format!("{}", errors)).style(error_style_cell),
            Cell::from(sparkline_str).style(sparkline_style),
            Cell::from(win_bar).style(if win_rate > 50.0 {
                success_style()
            } else if win_rate > 20.0 {
                highlight_style()
            } else {
                Style::default()
            }),
        ];

        let mut row = Row::new(cells).height(1);
        if idx == app.selected_idx {
            row = row.style(selected_row_style());
        }
        row
    });

    let active_providers = if app.mode == Mode::Hedged {
        format!(" (using {} providers)", app.provider_count)
    } else {
        String::new()
    };

    let title = format!(" Providers & Stats{} ", active_providers);

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Length(14),
            Constraint::Min(18),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(border_style()),
    )
    .column_spacing(2);

    frame.render_widget(table, area);
}

fn create_mini_sparkline(data: &[u64]) -> String {
    if data.is_empty() {
        return "───────────".to_string();
    }

    let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max_val = *data.iter().max().unwrap_or(&1);
    let min_val = *data.iter().min().unwrap_or(&0);
    let range = if max_val > min_val {
        max_val - min_val
    } else {
        1
    };

    let last_10: Vec<_> = data.iter().rev().take(11).rev().collect();

    last_10
        .iter()
        .map(|&&val| {
            let normalized = if range > 0 {
                ((val - min_val) as f64 / range as f64 * 7.0) as usize
            } else {
                0
            };
            chars[normalized.min(7)]
        })
        .collect()
}

fn draw_detail_panel(frame: &mut Frame, area: Rect, app: &App) {
    let constraints = if app.batch_mode {
        vec![
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(4),
            Constraint::Length(6),
        ]
    } else {
        vec![
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(6),
        ]
    };

    let detail_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    draw_config_section(frame, detail_layout[0], app);
    draw_last_call_section(frame, detail_layout[1], app);

    if app.batch_mode {
        draw_batch_progress(frame, detail_layout[2], app);
        draw_hedge_config_section(frame, detail_layout[3], app);
    } else {
        draw_hedge_config_section(frame, detail_layout[2], app);
    }
}

fn draw_config_section(frame: &mut Frame, area: Rect, app: &App) {
    let mode_str = app.mode_string();
    let mode_style = if app.mode == Mode::Hedged {
        highlight_style()
    } else {
        success_style()
    };

    let method_str = match app.method {
        Method::LatestBlockhash => "get_latest_blockhash",
        Method::GetAccount => "get_account",
    };

    let provider_str = app
        .selected_provider_id()
        .map(|id| id.0.to_string())
        .unwrap_or_else(|| "-".into());

    let batch_status = if app.batch_mode {
        format!("ON ({}/{})", app.batch_current, app.batch_count)
    } else {
        format!("OFF (count: {})", app.batch_count)
    };

    let batch_style = if app.batch_mode {
        success_style()
    } else {
        muted_style()
    };

    let text = vec![
        Line::from(vec![
            Span::raw("Mode    : ").style(muted_style()),
            Span::raw(mode_str).style(mode_style),
        ]),
        Line::from(vec![
            Span::raw("Method  : ").style(muted_style()),
            Span::raw(method_str).style(Style::default().fg(TEXT_COLOR)),
        ]),
        Line::from(vec![
            Span::raw("Provider: ").style(muted_style()),
            Span::raw(provider_str).style(Style::default().fg(TEXT_COLOR)),
        ]),
        Line::from(vec![
            Span::raw("Batch   : ").style(muted_style()),
            Span::raw(batch_status).style(batch_style),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title("   Configuration ")
                .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(border_style()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_last_call_section(frame: &mut Frame, area: Rect, app: &App) {
    let last_provider_str = app
        .last_provider
        .map(|id| id.0.to_string())
        .unwrap_or_else(|| "-".into());

    let last_latency_str = app
        .last_latency_ms
        .map(|ms| format!("{:.1} ms", ms))
        .unwrap_or_else(|| "-".into());

    let latency_style = app
        .last_latency_ms
        .map(|ms| {
            if ms < 200.0 {
                success_style()
            } else if ms < 500.0 {
                highlight_style()
            } else {
                error_style()
            }
        })
        .unwrap_or_else(muted_style);

    let text = vec![
        Line::from(vec![
            Span::raw("Result  : ").style(muted_style()),
            Span::raw(&app.last_message).style(Style::default().fg(TEXT_COLOR)),
        ]),
        Line::from(vec![
            Span::raw("Winner  : ").style(muted_style()),
            Span::raw(last_provider_str).style(success_style()),
        ]),
        Line::from(vec![
            Span::raw("Latency : ").style(muted_style()),
            Span::raw(last_latency_str).style(latency_style),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title("  Last Call ")
                .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(border_style()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_batch_progress(frame: &mut Frame, area: Rect, app: &App) {
    let progress = if app.batch_count > 0 {
        (app.batch_current as f64 / app.batch_count as f64 * 100.0) as u16
    } else {
        0
    };

    let label = format!("{}/{}", app.batch_current, app.batch_count);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title("  Batch Progress ")
                .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(border_style()),
        )
        .gauge_style(success_style())
        .percent(progress)
        .label(label);

    frame.render_widget(gauge, area);
}

fn draw_hedge_config_section(frame: &mut Frame, area: Rect, app: &App) {
    let total_providers = app.providers.len();
    let active_count = if app.mode == Mode::Hedged {
        app.provider_count.to_string()
    } else {
        "1".to_string()
    };

    let text = vec![Line::from(vec![
        Span::raw("Total    : ").style(muted_style()),
        Span::raw(total_providers.to_string()).style(Style::default().fg(TEXT_COLOR)),
        Span::raw("  │  Active: ").style(muted_style()),
        Span::raw(&active_count).style(if app.mode == Mode::Hedged {
            success_style()
        } else {
            Style::default().fg(TEXT_COLOR)
        }),
        Span::raw("  │  Delay: ").style(muted_style()),
        Span::raw("50ms").style(Style::default().fg(TEXT_COLOR)),
    ])];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title("  Hedge Config ")
                .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(border_style()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_keybinds(frame: &mut Frame, area: Rect) {
    let keybinds = vec![
        Line::from(vec![
            Span::raw("  ").style(muted_style()),
            Span::raw("↑/↓").style(highlight_style()),
            Span::raw(" Select provider  │  ").style(muted_style()),
            Span::raw("Space").style(highlight_style()),
            Span::raw(" Quick test selected  │  ").style(muted_style()),
            Span::raw("Tab").style(highlight_style()),
            Span::raw(" Toggle mode").style(muted_style()),
        ]),
        Line::from(vec![
            Span::raw("  ").style(muted_style()),
            Span::raw("+/-").style(highlight_style()),
            Span::raw(" Provider count  │  ").style(muted_style()),
            Span::raw("r").style(highlight_style()),
            Span::raw(" Run call      │  ").style(muted_style()),
            Span::raw("m").style(highlight_style()),
            Span::raw(" Toggle method  │  ").style(muted_style()),
            Span::raw("b").style(highlight_style()),
            Span::raw(" Toggle batch").style(muted_style()),
        ]),
        Line::from(vec![
            Span::raw("  ").style(muted_style()),
            Span::raw(",/.").style(highlight_style()),
            Span::raw(" Batch count     │  ").style(muted_style()),
            Span::raw("s").style(highlight_style()),
            Span::raw(" Reset stats   │  ").style(muted_style()),
            Span::raw("q").style(highlight_style()),
            Span::raw(" Quit").style(muted_style()),
        ]),
    ];

    let paragraph = Paragraph::new(keybinds)
        .block(
            Block::default()
                .title("  Keybinds ")
                .title_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(border_style()),
        )
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
