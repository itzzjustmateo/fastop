use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::theme::{ACCENT, MUTED};

impl App {
    pub(crate) fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let key = |k: &'static str, desc: &'static str| {
            [
                Span::styled(format!(" {k} "), Style::default().fg(ACCENT).bold()),
                Span::styled(format!("{desc}  "), Style::default().fg(MUTED)),
            ]
        };

        let mut spans: Vec<Span> = Vec::new();

        if area.width >= 40 {
            spans.extend(key("↑/↓", "navigate"));
        }
        if area.width >= 26 {
            spans.extend(key("s", "sort"));
        }
        spans.extend(key("q", "quit"));

        frame.render_widget(Line::from(spans), area);
    }
}
