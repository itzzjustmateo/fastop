use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, ProcessesToUpdate, System};

const TICK_RATE: Duration = Duration::from_millis(500);
const MUTED: Color = Color::DarkGray;
const ACCENT: Color = Color::Cyan;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortBy {
    Cpu,
    Memory,
}

impl SortBy {
    fn label(self) -> &'static str {
        match self {
            SortBy::Cpu => "CPU",
            SortBy::Memory => "MEM",
        }
    }
}

struct ProcessRow {
    pid: u32,
    name: String,
    cpu: f32,
    memory: u64,
}

struct App {
    running: bool,
    system: System,
    host_name: String,
    os_name: String,
    processes: Vec<ProcessRow>,
    table_state: TableState,
    sort: SortBy,
}

fn main() -> std::io::Result<()> {
    ratatui::run(|terminal| App::new().run(terminal))
}

impl App {
    fn new() -> Self {
        let mut system = System::new_all();
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();

        let mut app = Self {
            running: true,
            system,
            host_name: System::host_name().unwrap_or_else(|| "unknown".into()),
            os_name: System::long_os_version().unwrap_or_else(|| "unknown OS".into()),
            processes: Vec::new(),
            table_state: TableState::default(),
            sort: SortBy::Cpu,
        };

        app.on_tick();
        app
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let mut last_tick = Instant::now();

        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());

            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
            {
                self.handle_key(key);
            }

            if last_tick.elapsed() >= TICK_RATE {
                self.on_tick();
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Char('s') => {
                self.sort = match self.sort {
                    SortBy::Cpu => SortBy::Memory,
                    SortBy::Memory => SortBy::Cpu,
                };
                self.update_process_list();
            }
            _ => {}
        }
    }

    fn select_next(&mut self) {
        let next = match self.table_state.selected() {
            Some(index) if index + 1 < self.processes.len() => index + 1,
            _ => 0,
        };

        if !self.processes.is_empty() {
            self.table_state.select(Some(next));
        }
    }

    fn select_previous(&mut self) {
        let previous = match self.table_state.selected() {
            Some(0) | None => self.processes.len().saturating_sub(1),
            Some(index) => index - 1,
        };

        if !self.processes.is_empty() {
            self.table_state.select(Some(previous));
        }
    }

    fn on_tick(&mut self) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.update_process_list();
    }

    fn update_process_list(&mut self) {
        let mut processes: Vec<ProcessRow> = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessRow {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().into_owned(),
                cpu: process.cpu_usage(),
                memory: process.memory(),
            })
            .collect();

        processes.sort_by(|a, b| match self.sort {
            SortBy::Cpu => b
                .cpu
                .total_cmp(&a.cpu)
                .then_with(|| b.memory.cmp(&a.memory)),
            SortBy::Memory => b
                .memory
                .cmp(&a.memory)
                .then_with(|| b.cpu.total_cmp(&a.cpu)),
        });

        self.processes = processes;

        let selected = self
            .table_state
            .selected()
            .unwrap_or(0)
            .min(self.processes.len().saturating_sub(1));

        self.table_state
            .select((!self.processes.is_empty()).then_some(selected));
    }

    fn render(&mut self, frame: &mut Frame) {
        let [header, top, history, processes, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        let [cpu, memory] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top);

        self.render_header(frame, header);
        self.render_cpu(frame, cpu);
        self.render_memory(frame, memory);
        frame.render_widget(panel("History"), history);
        self.render_processes(frame, processes);
        self.render_footer(frame, footer);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let uptime = System::uptime();
        let hours = uptime / 3600;
        let minutes = uptime / 60 % 60;

        let block = panel("");
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let [brand, system, uptime] = Layout::horizontal([
            Constraint::Length(10),
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

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let key = |k: &'static str, desc: &'static str| {
            [
                Span::styled(format!(" {k} "), Style::default().fg(ACCENT).bold()),
                Span::styled(format!("{desc}  "), Style::default().fg(MUTED)),
            ]
        };

        let spans: Vec<Span> = [key("↑/↓", "navigate"), key("s", "sort"), key("q", "quit")]
            .into_iter()
            .flatten()
            .collect();

        frame.render_widget(Line::from(spans), area);
    }

    fn render_cpu(&self, frame: &mut Frame, area: Rect) {
        let average = self.system.global_cpu_usage();
        let block = panel(&format!("CPU {average:.0}%"));
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let mut lines = Vec::new();

        for pair in self.system.cpus().chunks(2) {
            let mut spans = Vec::new();

            for (index, cpu) in pair.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::raw("  "));
                }

                let usage = cpu.cpu_usage();

                spans.push(Span::styled(
                    format!("C{:02} ", lines.len() * 2 + index),
                    Style::default().fg(MUTED),
                ));

                spans.extend(bar_spans(usage, 5));

                spans.push(Span::styled(
                    format!(" {:>3.0}%", usage),
                    Style::default().fg(usage_color(usage)).bold(),
                ));
            }

            lines.push(Line::from(spans));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_memory(&self, frame: &mut Frame, area: Rect) {
        let sys = &self.system;

        let block = panel("Memory");
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let ram_percent = percent(sys.used_memory(), sys.total_memory());
        let swap_percent = percent(sys.used_swap(), sys.total_swap());

        let mut ram_line = vec![Span::styled("RAM  ", Style::default().fg(MUTED).bold())];
        ram_line.extend(bar_spans(ram_percent, 10));
        ram_line.push(Span::styled(
            format!(" {:>3.0}%", ram_percent),
            Style::default().fg(usage_color(ram_percent)).bold(),
        ));

        let mut swap_line = vec![Span::styled("Swap ", Style::default().fg(MUTED).bold())];
        swap_line.extend(bar_spans(swap_percent, 10));
        swap_line.push(Span::styled(
            format!(" {:>3.0}%", swap_percent),
            Style::default().fg(usage_color(swap_percent)).bold(),
        ));

        let lines = vec![
            Line::from(ram_line),
            Line::from(format!(
                "      {:.1} / {:.1} GiB",
                to_gib(sys.used_memory()),
                to_gib(sys.total_memory())
            )),
            Line::from(""),
            Line::from(swap_line),
            Line::from(format!(
                "      {:.1} / {:.1} GiB",
                to_gib(sys.used_swap()),
                to_gib(sys.total_swap())
            )),
        ];

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_processes(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(
            "Processes · {} · {}",
            self.processes.len(),
            self.sort.label()
        );
        let block = panel(&title);
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let header = Row::new([
            Cell::from(Line::from("PID").alignment(Alignment::Right)),
            Cell::from("NAME"),
            Cell::from(Line::from("CPU%").alignment(Alignment::Right)),
            Cell::from(Line::from("MEM").alignment(Alignment::Right)),
        ])
        .style(Style::default().fg(MUTED).bold());

        let rows = self.processes.iter().map(|process| {
            Row::new([
                Cell::from(Line::from(process.pid.to_string()).alignment(Alignment::Right)),
                Cell::from(process.name.clone()),
                Cell::from(
                    Line::from(Span::styled(
                        format!("{:.1}", process.cpu),
                        Style::default().fg(usage_color(process.cpu)),
                    ))
                    .alignment(Alignment::Right),
                ),
                Cell::from(
                    Line::from(format!("{:.1} MiB", to_mib(process.memory)))
                        .alignment(Alignment::Right),
                ),
            ])
        });

        let widths = [
            Constraint::Length(7),
            Constraint::Fill(1),
            Constraint::Length(7),
            Constraint::Length(12),
        ];

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

fn panel(title: &str) -> Block<'static> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(MUTED);

    if title.is_empty() {
        block
    } else {
        block.title(Line::from(format!(" {title} ")).fg(ACCENT).bold())
    }
}

fn usage_color(usage: f32) -> Color {
    match usage {
        u if u >= 90.0 => Color::Red,
        u if u >= 70.0 => Color::Yellow,
        _ => Color::Green,
    }
}

fn bar_spans(usage: f32, width: usize) -> Vec<Span<'static>> {
    let usage = usage.clamp(0.0, 100.0);
    let filled = (usage / 100.0 * width as f32).round() as usize;
    let empty = width - filled;

    vec![
        Span::styled("█".repeat(filled), Style::default().fg(usage_color(usage))),
        Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
    ]
}

fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32 * 100.0
    }
}

fn to_gib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0_f64.powi(3)
}

fn to_mib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0_f64.powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_handles_zero_total() {
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(1024, 2048), 50.0);
    }

    #[test]
    fn usage_color_thresholds() {
        assert_eq!(usage_color(0.0), Color::Green);
        assert_eq!(usage_color(69.9), Color::Green);
        assert_eq!(usage_color(70.0), Color::Yellow);
        assert_eq!(usage_color(89.9), Color::Yellow);
        assert_eq!(usage_color(90.0), Color::Red);
    }

    #[test]
    fn renders_without_panicking() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        assert_eq!(terminal.backend().buffer().area.width, 120);
    }

    #[test]
    fn byte_conversions_are_exact() {
        assert_eq!(to_gib(1024_u64.pow(3)), 1.0);
        assert_eq!(to_mib(1024_u64.pow(2)), 1.0);
    }

    #[test]
    fn bar_spans_fill_proportionally() {
        let spans = bar_spans(50.0, 10);
        assert_eq!(spans[0].content.as_ref(), "█████");
        assert_eq!(spans[1].content.as_ref(), "░░░░░");
    }

    #[test]
    fn bar_spans_clamp_out_of_range_values() {
        assert_eq!(bar_spans(-10.0, 10)[0].content, "");
        assert_eq!(bar_spans(150.0, 10)[1].content, "");
    }
}
