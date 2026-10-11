use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::format::{percent, to_gib};
use crate::widgets::{meter, panel};

impl App {
    pub(crate) fn render_memory(&self, frame: &mut Frame, area: Rect) {
        let block = panel("Memory");
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let sys = self.collector.memory();

        let lines = vec![
            meter(
                "RAM",
                percent(sys.used, sys.total),
                &memory_detail(sys.used, sys.total),
                inner.width,
            ),
            meter(
                "Swap",
                percent(sys.used_swap, sys.total_swap),
                &memory_detail(sys.used_swap, sys.total_swap),
                inner.width,
            ),
        ];

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Formats a `used / total GiB` detail string for a meter.
fn memory_detail(used: u64, total: u64) -> String {
    format!("{:.1} / {:.1} GiB", to_gib(used), to_gib(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_memory_detail() {
        assert_eq!(
            memory_detail(1024_u64.pow(3), 2 * 1024_u64.pow(3)),
            "1.0 / 2.0 GiB"
        );
    }
}
