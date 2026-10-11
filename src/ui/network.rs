use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::format::{format_rate, format_rate_short};
use crate::network::NetworkRow;
use crate::theme::{ACCENT, MUTED, UPLOAD};
use crate::widgets::{panel, truncate};

impl App {
    pub(crate) fn render_network(&self, frame: &mut Frame, area: Rect) {
        let (received, transmitted) = self
            .collector
            .network_rows()
            .iter()
            .fold((0.0, 0.0), |(down, up), row| {
                (down + row.received, up + row.transmitted)
            });

        let title = if area.width >= 60 {
            format!(
                "Network · ↓ {} ↑ {}",
                format_rate(received),
                format_rate(transmitted)
            )
        } else {
            format!(
                "Network · ↓{} ↑{}",
                format_rate_short(received),
                format_rate_short(transmitted)
            )
        };

        let block = panel(&title);
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let rows = self.collector.network_rows();
        if rows.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No network interfaces",
                    Style::default().fg(MUTED),
                ))),
                inner,
            );
            return;
        }

        let compact = inner.width < 40;
        let name_width = if compact { 6 } else { 8 };

        let lines = rows
            .iter()
            .map(|row| network_line(row, name_width, compact))
            .collect::<Vec<_>>();

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Renders a single network interface line, compacting when space is tight.
fn network_line(row: &NetworkRow, name_width: usize, compact: bool) -> Line<'static> {
    let name = format!(
        "{:<width$}",
        truncate(&row.name, name_width),
        width = name_width
    );
    let down = Style::default().fg(ACCENT);
    let up = Style::default().fg(UPLOAD);

    if compact {
        Line::from(vec![
            Span::styled(name, Style::default().fg(Color::White)),
            Span::styled(format!("↓{}", format_rate_short(row.received)), down),
            Span::raw(" "),
            Span::styled(format!("↑{}", format_rate_short(row.transmitted)), up),
        ])
    } else {
        Line::from(vec![
            Span::styled(name, Style::default().fg(Color::White)),
            Span::styled(format!("↓ {:<10} ", format_rate(row.received)), down),
            Span::styled(format!("↑ {}", format_rate(row.transmitted)), up),
        ])
    }
}
