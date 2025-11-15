//! Color scheme and styling for the TUI dashboard.

use ratatui::prelude::*;

pub const HEADER_BG: Color = Color::Rgb(30, 30, 46);
pub const HEADER_FG: Color = Color::Rgb(180, 190, 254);

pub const TABLE_HEADER_FG: Color = Color::Rgb(137, 180, 250);

pub const SELECTED_BG: Color = Color::Rgb(137, 180, 250);
pub const SELECTED_FG: Color = Color::Black;

pub const SUCCESS_COLOR: Color = Color::Rgb(166, 227, 161);
pub const ERROR_COLOR: Color = Color::Rgb(243, 139, 168);
pub const WARNING_COLOR: Color = Color::Rgb(249, 226, 175);

pub const BORDER_COLOR: Color = Color::Rgb(69, 71, 90);
pub const TEXT_COLOR: Color = Color::Rgb(205, 214, 244);
pub const MUTED_COLOR: Color = Color::Rgb(127, 132, 156);

pub fn header_style() -> Style {
    Style::default()
        .bg(HEADER_BG)
        .fg(HEADER_FG)
        .add_modifier(Modifier::BOLD)
}

pub fn table_header_style() -> Style {
    Style::default()
        .fg(TABLE_HEADER_FG)
        .add_modifier(Modifier::BOLD)
}

pub fn selected_row_style() -> Style {
    Style::default()
        .bg(SELECTED_BG)
        .fg(SELECTED_FG)
        .add_modifier(Modifier::BOLD)
}

pub fn border_style() -> Style {
    Style::default().fg(BORDER_COLOR)
}

pub fn success_style() -> Style {
    Style::default()
        .fg(SUCCESS_COLOR)
        .add_modifier(Modifier::BOLD)
}

pub fn error_style() -> Style {
    Style::default()
        .fg(ERROR_COLOR)
        .add_modifier(Modifier::BOLD)
}

pub fn highlight_style() -> Style {
    Style::default()
        .fg(WARNING_COLOR)
        .add_modifier(Modifier::BOLD)
}

pub fn muted_style() -> Style {
    Style::default().fg(MUTED_COLOR)
}
