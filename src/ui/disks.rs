use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::format::{percent, to_gib};
use crate::theme::MUTED;
use crate::widgets::{meter, panel, truncate};

impl App {
    pub(crate) fn render_disks(&self, frame: &mut Frame, area: Rect) {
        let rows = self.collector.disk_rows();

        let block = panel("Disks");
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if rows.is_empty() {
            let message = Paragraph::new(Line::from(Span::styled(
                "No disks detected",
                Style::default().fg(MUTED),
            )));
            frame.render_widget(message, inner);
            return;
        }

        let lines = rows
            .iter()
            .map(|row| {
                let label = format!("{:<8}", truncate(&row.label, 8));
                let detail = format!(
                    "{:<14}",
                    format!("{:.0}/{:.0} GiB", to_gib(row.used), to_gib(row.total))
                );
                meter(&label, percent(row.used, row.total), &detail, inner.width)
            })
            .collect::<Vec<_>>();

        frame.render_widget(Paragraph::new(lines), inner);
    }
}
