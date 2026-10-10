use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, LineGauge, Paragraph, Row, Sparkline, Table};
use sysinfo::System;

use crate::app::App;
use crate::battery::{BatteryInfo, BatteryState, format_duration};
use crate::disks::disk_rows;
use crate::format::{format_memory, format_rate, format_rate_short, percent, to_gib};
use crate::network::NetworkRow;
use crate::processes::{ProcessColumn, SortBy, header_cell, process_columns};
use crate::theme::{ACCENT, MUTED, UPLOAD, charge_color, temperature_color, usage_color};
use crate::widgets::{bar_spans, bar_spans_colored, meter, panel, right, truncate};

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

        let batteries = self.battery.batteries();
        let show_battery = self.battery.present() && inner.width >= 80;

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
                    self.os_name.as_str(),
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled("  •  ", Style::default().fg(MUTED)),
                Span::styled(self.host_name.as_str(), Style::default().fg(MUTED)),
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

    pub(crate) fn render_cpu(&self, frame: &mut Frame, area: Rect) {
        let average = self.system.global_cpu_usage();
        let block = panel(&format!("CPU {average:.0}%"));
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let [body, temperature] = split_temperature(inner, self.cpu_temp.is_some());

        let name = Line::from(Span::styled(
            self.cpu_name.as_str(),
            Style::default().fg(Color::White),
        ));
        let per_line = (body.width as usize / 17).clamp(1, 4);
        let max_lines = body.height as usize;

        let mut lines = vec![name];

        for (chunk, cores) in self.system.cpus().chunks(per_line).enumerate() {
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
            self.cpu_temp,
            &self.cpu_temp_history.tail(temperature.width as usize),
        );
    }

    pub(crate) fn render_memory(&self, frame: &mut Frame, area: Rect) {
        let sys = &self.system;

        let block = panel("Memory");
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let lines = vec![
            meter(
                "RAM",
                percent(sys.used_memory(), sys.total_memory()),
                &format!(
                    "{:.1} / {:.1} GiB",
                    to_gib(sys.used_memory()),
                    to_gib(sys.total_memory())
                ),
                inner.width,
            ),
            meter(
                "Swap",
                percent(sys.used_swap(), sys.total_swap()),
                &format!(
                    "{:.1} / {:.1} GiB",
                    to_gib(sys.used_swap()),
                    to_gib(sys.total_swap())
                ),
                inner.width,
            ),
        ];

        frame.render_widget(Paragraph::new(lines), inner);
    }

    pub(crate) fn render_gpu(&self, frame: &mut Frame, area: Rect) {
        let block = match &self.gpu {
            Some(gpu) => panel(&format!("GPU {:.0}%", gpu.usage)),
            None => panel("GPU"),
        };
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let Some(gpu) = &self.gpu else {
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
            &self.gpu_temp_history.tail(temperature.width as usize),
        );
    }

    pub(crate) fn render_disks(&self, frame: &mut Frame, area: Rect) {
        let rows = disk_rows(&self.disks);

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

    pub(crate) fn render_network(&self, frame: &mut Frame, area: Rect) {
        let (received, transmitted) = self
            .network_rows
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

        if self.network_rows.is_empty() {
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

        let lines = self
            .network_rows
            .iter()
            .map(|row| network_line(row, name_width, compact))
            .collect::<Vec<_>>();

        frame.render_widget(Paragraph::new(lines), inner);
    }

    pub(crate) fn render_processes(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(
            "Processes · {} · {}",
            self.processes.len(),
            self.sort.label()
        );
        let block = panel(&title);
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let columns = process_columns(inner.width);
        let selected = self.table_state.selected();

        let header = Row::new(columns.iter().map(|column| {
            let line = match column {
                ProcessColumn::Pid => header_cell("PID", self.sort, SortBy::Pid, true),
                ProcessColumn::User => Line::from("USER"),
                ProcessColumn::Name => header_cell("NAME", self.sort, SortBy::Name, false),
                ProcessColumn::Cpu => header_cell("CPU%", self.sort, SortBy::Cpu, true),
                ProcessColumn::Memory => header_cell("MEM", self.sort, SortBy::Memory, true),
            };

            Cell::from(line)
        }))
        .style(Style::default().fg(MUTED).bold());

        let rows = self.processes.iter().enumerate().map(|(index, process)| {
            let cpu_style = if selected == Some(index) {
                Style::default()
            } else {
                Style::default().fg(usage_color(process.cpu))
            };

            Row::new(columns.iter().map(|column| {
                match column {
                    ProcessColumn::Pid => Cell::from(right(process.pid.to_string())),
                    ProcessColumn::User => Cell::from(truncate(&process.user, 10)),
                    ProcessColumn::Name => Cell::from(process.name.clone()),
                    ProcessColumn::Cpu => Cell::from(
                        Line::from(Span::styled(format!("{:.1}", process.cpu), cpu_style))
                            .alignment(Alignment::Right),
                    ),
                    ProcessColumn::Memory => Cell::from(right(format_memory(process.memory))),
                }
            }))
        });

        let widths = columns.iter().map(|column| match column {
            ProcessColumn::Pid => Constraint::Length(7),
            ProcessColumn::User => Constraint::Length(10),
            ProcessColumn::Name => Constraint::Fill(1),
            ProcessColumn::Cpu => Constraint::Length(7),
            ProcessColumn::Memory => Constraint::Length(12),
        });

        let table = Table::new(rows, widths)
            .header(header)
            .column_spacing(1)
            .highlight_symbol("❯ ")
            .row_highlight_style(
                Style::default()
                    .bg(ACCENT)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_stateful_widget(table, inner, &mut self.table_state);
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

/// Splits a panel into a body area and a temperature band
/// (empty when there is no sensor or no room).
fn split_temperature(inner: Rect, has_temperature: bool) -> [Rect; 2] {
    let graph_height = if has_temperature && inner.height >= 2 {
        inner.height.saturating_sub(1).min(3)
    } else {
        0
    };

    Layout::vertical([Constraint::Min(0), Constraint::Length(graph_height)]).areas(inner)
}

/// Draws a temperature line gauge above a sparkline of recent readings.
fn render_temperature_graph(frame: &mut Frame, area: Rect, current: Option<f32>, history: &[u64]) {
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
