use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{render_temperature_graph, split_temperature};
use crate::app::App;
use crate::theme::MUTED;
use crate::widgets::{meter, panel};

impl App {
    pub(crate) fn render_gpu(&self, frame: &mut Frame, area: Rect) {
        let gpu = self.collector.gpu();

        let block = match gpu {
            Some(gpu) => panel(&format!("GPU {:.0}%", gpu.usage)),
            None => panel("GPU"),
        };
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let Some(gpu) = gpu else {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No GPU detected",
                    Style::default().fg(MUTED),
                ))),
                inner,
            );
            return;
        };

        let [body, temperature] = split_temperature(inner, gpu.temperature.is_some());

        let lines = vec![
            Line::from(Span::styled(
                gpu.name.as_str(),
                Style::default().fg(Color::White),
            )),
            meter("GPU", gpu.usage, "", body.width),
        ];

        frame.render_widget(Paragraph::new(lines), body);

        render_temperature_graph(
            frame,
            temperature,
            gpu.temperature,
            &self
                .collector
                .gpu_temp_history()
                .tail(temperature.width as usize),
        );
    }
}
