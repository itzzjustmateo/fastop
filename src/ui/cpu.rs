use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{render_temperature_graph, split_temperature};
use crate::app::App;
use crate::theme::{MUTED, usage_color};
use crate::widgets::{bar_spans, panel};

impl App {
    pub(crate) fn render_cpu(&self, frame: &mut Frame, area: Rect) {
        let average = self.collector.global_cpu_usage();
        let block = panel(&format!("CPU {average:.0}%"));
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let [body, temperature] = split_temperature(inner, self.collector.cpu_temp().is_some());

        let name = Line::from(Span::styled(
            self.collector.cpu_name(),
            Style::default().fg(Color::White),
        ));
        let per_line = (body.width as usize / 17).clamp(1, 4);
        let max_lines = body.height as usize;

        let mut lines = vec![name];

        for (chunk, cores) in self.collector.cpus().chunks(per_line).enumerate() {
            if lines.len() >= max_lines {
                break;
            }

            let mut spans = Vec::new();

            for (index, cpu) in cores.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::raw("  "));
                }

                let usage = cpu.cpu_usage();

                spans.push(Span::styled(
                    format!("C{:02} ", chunk * per_line + index),
                    Style::default().fg(MUTED),
                ));

                spans.extend(bar_spans(usage, 6));

                spans.push(Span::styled(
                    format!(" {:>3.0}%", usage),
                    Style::default().fg(usage_color(usage)).bold(),
                ));
            }

            lines.push(Line::from(spans));
        }

        frame.render_widget(Paragraph::new(lines), body);

        render_temperature_graph(
            frame,
            temperature,
            self.collector.cpu_temp(),
            &self
                .collector
                .cpu_temp_history()
                .tail(temperature.width as usize),
        );
    }
}
