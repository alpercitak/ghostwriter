use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
};

use crate::modes::{RenderState, Status};

/// `seed` is shown in the title so any run can be reproduced; `looping` says
/// whether another run starts automatically after this one.
pub fn render(frame: &mut Frame, state: RenderState<'_>, seed: &str, looping: bool) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // log panel
            Constraint::Length(3), // progress bar
            Constraint::Length(1), // status line
        ])
        .split(area);

    let visible_height = chunks[0].height.saturating_sub(2) as usize;
    let start = state.log_lines.len().saturating_sub(visible_height);
    let items: Vec<ListItem> = state.log_lines[start..]
        .iter()
        .cloned()
        .map(ListItem::new)
        .collect();

    let log_block = Block::default()
        .title(format!(" ✦ ghostwriter — {} · seed {seed} ", state.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let log_widget = List::new(items).block(log_block);
    frame.render_widget(log_widget, chunks[0]);

    let (bar_color, bar_label) = match state.status {
        Status::Running => (Color::Cyan, format!("{:.0}%", state.progress * 100.0)),
        Status::Success => (Color::Green, "done".to_string()),
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(bar_color).bg(Color::Black))
        .ratio(state.progress.clamp(0.0, 1.0))
        .label(bar_label);
    frame.render_widget(gauge, chunks[1]);

    let status_text = match state.status {
        Status::Running => format!(" running  {}s elapsed    q to quit", state.elapsed_secs),
        Status::Success if looping => {
            format!(
                " {} — next run starting…    q to quit",
                state.success_message
            )
        }
        Status::Success => format!(" {} — press any key to exit", state.success_message),
    };

    let status_style = match state.status {
        Status::Running => Style::default().fg(Color::DarkGray),
        Status::Success => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    };

    let status_widget = Paragraph::new(status_text).style(status_style);
    frame.render_widget(status_widget, chunks[2]);
}
