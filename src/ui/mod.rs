//! Rendering for every panel.
//!
//! Each panel lives in its own module and is implemented as an inherent
//! [`App`] method, so the renderers read state without making it `pub`.
//! Renderers are pure: they never mutate the collector.

mod cpu;
mod disks;
mod footer;
mod gpu;
mod header;
mod memory;
mod network;
mod processes;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{LineGauge, Sparkline};

use crate::app::App;
use crate::layout::PanelKind;
use crate::theme::{MUTED, temperature_color};

impl App {
    /// Dispatches to the renderer for a single summary panel.
    pub(crate) fn render_panel(&self, frame: &mut Frame, kind: PanelKind, area: Rect) {
        match kind {
            PanelKind::Cpu => self.render_cpu(frame, area),
            PanelKind::Memory => self.render_memory(frame, area),
            PanelKind::Gpu => self.render_gpu(frame, area),
            PanelKind::Disks => self.render_disks(frame, area),
            PanelKind::Network => self.render_network(frame, area),
        }
    }
}

/// Splits a panel into a body area and a temperature band
/// (empty when there is no sensor or no room).
pub(super) fn split_temperature(inner: Rect, has_temperature: bool) -> [Rect; 2] {
    let graph_height = if has_temperature && inner.height >= 2 {
        inner.height.saturating_sub(1).min(3)
    } else {
        0
    };

    Layout::vertical([Constraint::Min(0), Constraint::Length(graph_height)]).areas(inner)
}

/// Draws a temperature line gauge above a sparkline of recent readings.
pub(super) fn render_temperature_graph(
    frame: &mut Frame,
    area: Rect,
    current: Option<f32>,
    history: &[u64],
) {
    let Some(current) = current else {
        return;
    };

    if area.height == 0 || area.width == 0 {
        return;
    }

    let color = temperature_color(current);
    let [gauge_area, graph_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);

    frame.render_widget(
        LineGauge::default()
            .ratio((f64::from(current) / 100.0).clamp(0.0, 1.0))
            .label(format!("{current:.0}°C"))
            .filled_style(Style::default().fg(color))
            .unfilled_style(Style::default().fg(MUTED)),
        gauge_area,
    );

    if graph_area.height > 0 && !history.is_empty() {
        frame.render_widget(
            Sparkline::default()
                .data(history)
                .max(100)
                .style(Style::default().fg(color)),
            graph_area,
        );
    }
}
