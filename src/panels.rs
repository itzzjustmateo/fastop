use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};
use sysinfo::System;

use crate::app::App;
use crate::disks::disk_rows;
use crate::format::{format_memory, format_rate, format_rate_short, percent, to_gib};
use crate::network::NetworkRow;
use crate::processes::{ProcessColumn, SortBy, header_cell, process_columns};
use crate::sensors::cpu_temperature;
use crate::theme::{ACCENT, MUTED, UPLOAD, usage_color};
use crate::widgets::{bar_spans, meter, panel, right, truncate};

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

        let [brand, system, uptime] = Layout::horizontal([
            Constraint::Length(9),
            Constraint::Min(1),
            Constraint::Length(13),
        ])
        .areas(inner);

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
        let title = match cpu_temperature(&self.components) {
            Some(temperature) => format!("CPU {average:.0}% · {temperature:.0}°C"),
            None => format!("CPU {average:.0}%"),
        };
        let block = panel(&title);
        let inner = block.inner(area);

        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let name = Line::from(Span::styled(
            self.cpu_name.as_str(),
            Style::default().fg(Color::White),
        ));
        let per_line = (inner.width as usize / 17).clamp(1, 4);
        let max_lines = inner.height as usize;

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

        frame.render_widget(Paragraph::new(lines), inner);
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
            Some(gpu) => {
                let title = match gpu.temperature {
                    Some(temperature) => {
                        format!("GPU {:.0}% · {temperature:.0}°C", gpu.usage)
                    }
                    None => format!("GPU {:.0}%", gpu.usage),
                };
                panel(&title)
            }
            None => panel("GPU"),
        };
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let Some(gpu) = &self.gpu else {
            let message = Paragraph::new(Line::from(Span::styled(
                "No GPU detected",
                Style::default().fg(MUTED),
            )));
            frame.render_widget(message, inner);
            return;
        };

        let lines = vec![
            Line::from(Span::styled(
                gpu.name.as_str(),
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            meter("GPU", gpu.usage, "", inner.width),
        ];

        frame.render_widget(Paragraph::new(lines), inner);
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
