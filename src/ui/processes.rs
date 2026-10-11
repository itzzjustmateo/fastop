use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};

use crate::app::App;
use crate::format::format_memory;
use crate::processes::{ProcessColumn, SortBy, header_cell, process_columns};
use crate::theme::{ACCENT, MUTED, usage_color};
use crate::widgets::{panel, right, truncate};

impl App {
    pub(crate) fn render_processes(&mut self, frame: &mut Frame, area: Rect) {
        let processes = self.collector.process_rows();

        let title = format!("Processes · {} · {}", processes.len(), self.sort.label());
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

        let rows = processes.iter().enumerate().map(|(index, process)| {
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
