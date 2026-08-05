use crate::protocol::RefreshStep;
use crate::ui::ACCENT;
use crate::ui::helpers::truncate;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const SPINNER_FRAMES: [&str; 4] = ["\u{25d0}", "\u{25d3}", "\u{25d1}", "\u{25d2}"];

pub(crate) fn draw_loading(frame: &mut Frame, tick: usize, steps: &[RefreshStep]) {
    let area = frame.area();
    let content_height = 6_u16.saturating_add(steps.len() as u16);
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_height.min(area.height)),
        Constraint::Fill(1),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(48.min(area.width)),
        Constraint::Fill(1),
    ])
    .split(vertical[1]);

    let spinner = SPINNER_FRAMES[tick % SPINNER_FRAMES.len()];
    let mut content = vec![
        Line::from(Span::styled(
            spinner,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "pert",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "mux",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "loading dashboard...",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];
    let current_step = steps
        .iter()
        .position(|step| step.total > 0 && step.done < step.total);
    content.extend(
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| status_line(step, current_step == Some(index))),
    );

    frame.render_widget(
        Paragraph::new(content).alignment(Alignment::Center),
        horizontal[1],
    );
}

fn status_line(step: &RefreshStep, current: bool) -> Line<'static> {
    let complete = step.total == 0 || step.done >= step.total;
    let (marker, color) = if complete {
        ("✓", Color::Green)
    } else if current {
        ("›", ACCENT)
    } else {
        ("○", Color::DarkGray)
    };
    let label_color = if current || complete {
        Color::White
    } else {
        Color::DarkGray
    };
    let label = truncate(&step.label, 28);
    let count = if step.total > 1 {
        format!("  {}/{}", step.done, step.total)
    } else {
        String::new()
    };

    Line::from(vec![
        Span::styled(format!("{} ", marker), Style::default().fg(color)),
        Span::styled(label, Style::default().fg(label_color)),
        Span::styled(count, Style::default().fg(Color::DarkGray)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_shows_progress_counts_for_multi_item_steps() {
        let line = status_line(
            &RefreshStep {
                label: "Pulling forge APIs".to_string(),
                done: 2,
                total: 4,
            },
            true,
        );

        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(text, "› Pulling forge APIs  2/4");
    }

    #[test]
    fn status_line_marks_completed_steps() {
        let line = status_line(
            &RefreshStep {
                label: "Loading worktrees".to_string(),
                done: 3,
                total: 3,
            },
            false,
        );

        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(text, "✓ Loading worktrees  3/3");
    }
}
