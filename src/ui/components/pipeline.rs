use crate::gitlab::types::PipelineJob;
use crate::ui::ACCENT;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub(crate) fn render_pipeline_dots(jobs: &[PipelineJob]) -> Vec<Line<'static>> {
    let mut stages: Vec<(String, Vec<&PipelineJob>)> = Vec::new();
    for job in jobs {
        if let Some(existing) = stages.iter_mut().find(|(s, _)| s == &job.stage) {
            existing.1.push(job);
        } else {
            stages.push((job.stage.clone(), vec![job]));
        }
    }

    let mut spans: Vec<Span> = vec![Span::styled(
        "  jobs       ",
        Style::default().fg(Color::DarkGray),
    )];
    for (i, (_stage, stage_jobs)) in stages.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default()));
        }
        for job in stage_jobs {
            spans.push(Span::styled(
                "\u{25cf}",
                Style::default().fg(job_status_color(&job.status)),
            ));
        }
    }

    let mut lines = vec![Line::from(spans)];

    let failed: Vec<&PipelineJob> = jobs
        .iter()
        .filter(|j| j.status == "failed" && !j.allow_failure)
        .collect();
    if !failed.is_empty() {
        let names: String = failed
            .iter()
            .map(|j| j.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(Line::from(vec![
            Span::styled("             \u{2717} ", Style::default().fg(Color::Red)),
            Span::styled(names, Style::default().fg(Color::Red)),
        ]));
    }

    lines
}

fn job_status_color(status: &str) -> Color {
    match status {
        "success" => Color::Green,
        "failed" => Color::Red,
        "running" => ACCENT,
        "pending" | "preparing" | "waiting_for_resource" => Color::Yellow,
        "manual" => Color::Rgb(128, 90, 213),
        "canceled" | "canceling" | "skipped" => Color::DarkGray,
        "created" => Color::Gray,
        _ => Color::Gray,
    }
}
