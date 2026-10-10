use ratatui::layout::Alignment;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding};

use crate::theme::{ACCENT, MUTED, usage_color};

/// A rounded panel with optional accent title.
pub(crate) fn panel(title: &str) -> Block<'static> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(MUTED)
        .padding(Padding::horizontal(1));

    if title.is_empty() {
        block
    } else {
        block.title(Line::from(format!(" {title} ")).fg(ACCENT).bold())
    }
}

/// Smooth horizontal bar using full, partial and empty blocks.
pub(crate) fn bar_spans(usage: f32, width: usize) -> Vec<Span<'static>> {
    const PARTIALS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

    let usage = usage.clamp(0.0, 100.0);
    let eighths = (usage / 100.0 * (width * 8) as f32).round() as usize;
    let full = eighths / 8;
    let remainder = eighths % 8;
    let empty = width.saturating_sub(full + usize::from(remainder > 0));

    let mut spans = Vec::with_capacity(3);

    if full > 0 {
        spans.push(Span::styled(
            "█".repeat(full),
            Style::default().fg(usage_color(usage)),
        ));
    }
    if remainder > 0 {
        spans.push(Span::styled(
            PARTIALS[remainder - 1].to_string(),
            Style::default().fg(usage_color(usage)),
        ));
    }
    if empty > 0 {
        spans.push(Span::styled(
            "░".repeat(empty),
            Style::default().fg(Color::DarkGray),
        ));
    }

    spans
}

/// A labelled percentage bar, dropping the detail text when space runs out.
pub(crate) fn meter(label: &str, usage: f32, detail: &str, width: u16) -> Line<'static> {
    const MIN_BAR: usize = 6;

    let label = format!("{label:<6}");
    let percent = format!(" {:>3.0}%", usage);
    let mut detail = if detail.is_empty() {
        String::new()
    } else {
        format!("  {detail}")
    };

    let width = width as usize;
    let fixed = label.chars().count() + percent.chars().count();

    // Drop the detail on narrow panels so the meter stays readable.
    if width < fixed + MIN_BAR + detail.chars().count() {
        detail.clear();
    }

    let bar_width = width.saturating_sub(fixed + detail.chars().count()).max(1);

    let mut spans = vec![Span::styled(label, Style::default().fg(MUTED).bold())];
    spans.extend(bar_spans(usage, bar_width));
    spans.push(Span::styled(
        percent,
        Style::default().fg(usage_color(usage)).bold(),
    ));
    if !detail.is_empty() {
        spans.push(Span::styled(detail, Style::default().fg(MUTED)));
    }

    Line::from(spans)
}

/// A single-line, right-aligned text cell.
pub(crate) fn right(text: impl Into<String>) -> Line<'static> {
    Line::from(text.into()).alignment(Alignment::Right)
}

/// Truncates `text` to `width` characters, appending an ellipsis when cut.
pub(crate) fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        let mut result: String = text.chars().take(width.saturating_sub(1)).collect();
        result.push('…');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar_text(usage: f32, width: usize) -> String {
        bar_spans(usage, width)
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn bar_spans_fill_proportionally() {
        assert_eq!(bar_text(50.0, 10), "█████░░░░░");
        assert_eq!(bar_text(50.0, 10).chars().count(), 10);
    }

    #[test]
    fn bar_spans_clamp_out_of_range_values() {
        assert_eq!(bar_text(-10.0, 10), "░░░░░░░░░░");
        assert_eq!(bar_text(150.0, 10), "██████████");
    }

    #[test]
    fn bar_spans_render_partial_blocks() {
        // 5% of a 10-cell bar is half of one cell.
        assert_eq!(bar_text(5.0, 10), "▌░░░░░░░░░");
        assert_eq!(bar_text(5.0, 10).chars().count(), 10);
    }

    #[test]
    fn truncate_keeps_short_and_shortens_long() {
        assert_eq!(truncate("short", 8), "short");
        assert_eq!(truncate("/very/long/mount", 8), "/very/l…");
    }
}
