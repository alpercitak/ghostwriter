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

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, text::Line};

    use super::render;
    use crate::modes::{RenderState, Status};

    /// Render one frame and return the screen as plain text rows.
    fn screen(state: RenderState<'_>, seed: &str, looping: bool, w: u16, h: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| render(frame, state, seed, looping))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    fn log(n: usize) -> Vec<Line<'static>> {
        (1..=n).map(|i| Line::raw(format!("line {i}"))).collect()
    }

    fn state<'a>(lines: &'a [Line<'static>], progress: f64, status: Status) -> RenderState<'a> {
        RenderState {
            title: "webpack build",
            success_message: "build succeeded",
            log_lines: lines,
            progress,
            status,
            elapsed_secs: 7,
        }
    }

    #[test]
    fn running_frame() {
        let lines = log(3);
        let expected = [
            "┌ ✦ ghostwriter — webpack build · seed quiet-ledger ───────┐",
            "│line 1                                                    │",
            "│line 2                                                    │",
            "│line 3                                                    │",
            "│                                                          │",
            "└──────────────────────────────────────────────────────────┘",
            "┌──────────────────────────────────────────────────────────┐",
            "│████████████████████████   42%                            │",
            "└──────────────────────────────────────────────────────────┘",
            " running  7s elapsed    q to quit                           ",
        ];
        assert_eq!(
            screen(
                state(&lines, 0.42, Status::Running),
                "quiet-ledger",
                false,
                60,
                10
            ),
            expected
        );
    }

    #[test]
    fn finished_frame_in_loop_mode_scrolls_log() {
        let lines = log(12);
        let expected = [
            "┌ ✦ ghostwriter — webpack build · seed quiet-ledger ───────┐",
            "│line 9                                                    │",
            "│line 10                                                   │",
            "│line 11                                                   │",
            "│line 12                                                   │",
            "└──────────────────────────────────────────────────────────┘",
            "┌──────────────────────────────────────────────────────────┐",
            "│███████████████████████████done ██████████████████████████│",
            "└──────────────────────────────────────────────────────────┘",
            " build succeeded — next run starting…    q to quit          ",
        ];
        assert_eq!(
            screen(
                state(&lines, 1.0, Status::Success),
                "quiet-ledger",
                true,
                60,
                10
            ),
            expected
        );
    }

    #[test]
    fn finished_frame() {
        let lines = log(12);
        let expected = [
            "┌ ✦ ghostwriter — webpack build · seed quiet-ledger ───────┐",
            "│line 9                                                    │",
            "│line 10                                                   │",
            "│line 11                                                   │",
            "│line 12                                                   │",
            "└──────────────────────────────────────────────────────────┘",
            "┌──────────────────────────────────────────────────────────┐",
            "│███████████████████████████done ██████████████████████████│",
            "└──────────────────────────────────────────────────────────┘",
            " build succeeded — press any key to exit                    ",
        ];
        assert_eq!(
            screen(
                state(&lines, 1.0, Status::Success),
                "quiet-ledger",
                false,
                60,
                10
            ),
            expected
        );
    }
}
