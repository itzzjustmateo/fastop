use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sysinfo::System;

use crate::app::App;
use crate::battery::{BatteryInfo, BatteryState, format_duration};
use crate::theme::{ACCENT, MUTED, charge_color};
use crate::widgets::{bar_spans_colored, panel};

impl App {
    pub(crate) fn render_header(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let uptime = System::uptime();
        let hours = uptime / 3600;
        let minutes = uptime / 60 % 60;

        let inner = if area.height >= 3 {
            let block = panel("");
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        } else {
            area
        };

        const BRAND_WIDTH: u16 = 9;
        const BATTERY_WIDTH: u16 = 30;
        const UPTIME_WIDTH: u16 = 13;

        let battery = self.collector.battery();
        let batteries = battery.batteries();
        let show_battery = battery.present() && inner.width >= 80;

        let constraints = if show_battery {
            vec![
                Constraint::Length(BRAND_WIDTH),
                Constraint::Min(1),
                Constraint::Length(BATTERY_WIDTH),
                Constraint::Length(UPTIME_WIDTH),
            ]
        } else {
            vec![
                Constraint::Length(BRAND_WIDTH),
                Constraint::Min(1),
                Constraint::Length(UPTIME_WIDTH),
            ]
        };

        let chunks = Layout::horizontal(constraints).split(inner);
        let brand = chunks[0];
        let system = chunks[1];
        let uptime = chunks[chunks.len() - 1];

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("FASTOP", Style::default().fg(ACCENT).bold()),
                Span::styled(" • ", Style::default().fg(MUTED)),
            ])),
            brand,
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    self.collector.os_name(),
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled("  •  ", Style::default().fg(MUTED)),
                Span::styled(self.collector.host_name(), Style::default().fg(MUTED)),
            ])),
            system,
        );

        if show_battery {
            frame.render_widget(Paragraph::new(battery_line(&batteries[0])), chunks[2]);
        }

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("↑ ", Style::default().fg(ACCENT)),
                Span::styled(
                    format!("{hours}h {minutes}m"),
                    Style::default().fg(Color::White).bold(),
                ),
            ]))
            .alignment(Alignment::Right),
            uptime,
        );
    }
}

/// Builds the compact battery indicator shown in the header bar.
fn battery_line(battery: &BatteryInfo) -> Line<'static> {
    let color = charge_color(battery.percentage);

    let mut spans = vec![Span::styled("BATT ", Style::default().fg(MUTED).bold())];
    spans.extend(bar_spans_colored(battery.percentage, 6, color));
    spans.push(Span::styled(
        format!(" {:>3.0}%", battery.percentage),
        Style::default().fg(color).bold(),
    ));

    let detail = battery
        .remaining()
        .map(format_duration)
        .or_else(|| battery.temperature.map(|celsius| format!("{celsius:.0}°C")))
        .unwrap_or_else(|| battery.state.label().to_string());
    spans.push(Span::styled(
        format!(" {detail}"),
        Style::default().fg(MUTED),
    ));

    if battery.state == BatteryState::Charging {
        spans.push(Span::styled(" +", Style::default().fg(Color::LightGreen)));
    }

    Line::from(spans)
}
