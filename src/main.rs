use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Components, MINIMUM_CPU_UPDATE_INTERVAL, ProcessesToUpdate, System};

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

struct Gpu {
    name: String,
    usage: f32,
    temperature: Option<f32>,
    backend: GpuBackend,
}

enum GpuBackend {
    Sysfs {
        usage_path: PathBuf,
        temperature_path: Option<PathBuf>,
    },
    NvidiaSmi(NvidiaShared),
}

#[derive(Clone, Default)]
struct NvidiaShared(Arc<Mutex<Option<NvidiaSample>>>);

#[derive(Clone, Copy)]
struct NvidiaSample {
    usage: f32,
    temperature: Option<f32>,
}

struct App {
    running: bool,
    system: System,
    components: Components,
    host_name: String,
    os_name: String,
    cpu_name: String,
    gpu: Option<Gpu>,
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

        let cpu_name = system
            .cpus()
            .first()
            .map_or_else(|| "Unknown CPU".to_string(), |cpu| cpu.brand().to_string());
        let gpu = Gpu::detect();

        let mut app = Self {
            running: true,
            system,
            components: Components::new_with_refreshed_list(),
            host_name: System::host_name().unwrap_or_else(|| "unknown".into()),
            os_name: System::long_os_version().unwrap_or_else(|| "unknown OS".into()),
            cpu_name,
            gpu,
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
        self.components.refresh(false);

        if let Some(gpu) = &mut self.gpu {
            gpu.refresh();
        }

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
        let [header, top, processes, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(12),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        let [cpu, memory, gpu] = Layout::horizontal([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .areas(top);

        self.render_header(frame, header);
        self.render_cpu(frame, cpu);
        self.render_memory(frame, memory);
        self.render_gpu(frame, gpu);
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
                Span::styled(" FASTOP", Style::default().fg(ACCENT).bold()),
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
        let title = match cpu_temperature(&self.components) {
            Some(temperature) => format!("CPU {average:.0}% · {temperature:.0}°C"),
            None => format!("CPU {average:.0}%"),
        };
        let block = panel(&title);
        let inner = block.inner(area);

        frame.render_widget(block, area);

        let mut lines = vec![Line::from(Span::styled(
            self.cpu_name.as_str(),
            Style::default().fg(MUTED),
        ))];

        for (chunk, pair) in self.system.cpus().chunks(2).enumerate() {
            let mut spans = Vec::new();

            for (index, cpu) in pair.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::raw("  "));
                }

                let usage = cpu.cpu_usage();

                spans.push(Span::styled(
                    format!("C{:02} ", chunk * 2 + index),
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

    fn render_gpu(&self, frame: &mut Frame, area: Rect) {
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

        let mut usage = vec![Span::styled("GPU  ", Style::default().fg(MUTED).bold())];
        usage.extend(bar_spans(gpu.usage, 12));
        usage.push(Span::styled(
            format!(" {:>3.0}%", gpu.usage),
            Style::default().fg(usage_color(gpu.usage)).bold(),
        ));

        let lines = vec![
            Line::from(Span::styled(
                gpu.name.as_str(),
                Style::default().fg(Color::White).bold(),
            )),
            Line::from(""),
            Line::from(usage),
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

impl Gpu {
    fn detect() -> Option<Self> {
        Self::detect_sysfs().or_else(Self::detect_nvidia)
    }

    fn detect_sysfs() -> Option<Self> {
        let entries = std::fs::read_dir("/sys/class/drm").ok()?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if !is_drm_card(&name) {
                continue;
            }

            let device = entry.path().join("device");
            if !device.is_dir() {
                continue;
            }

            let usage_path = device.join("gpu_busy_percent");
            if !usage_path.exists() {
                continue;
            }

            let mut gpu = Gpu {
                name: gpu_name_from_device(&device).unwrap_or_else(|| "Unknown GPU".to_string()),
                usage: 0.0,
                temperature: None,
                backend: GpuBackend::Sysfs {
                    usage_path,
                    temperature_path: gpu_temperature_path(&device),
                },
            };
            gpu.refresh();

            return Some(gpu);
        }

        None
    }

    fn detect_nvidia() -> Option<Self> {
        let first = query_nvidia_smi()?;
        let shared = spawn_nvidia_poller();

        Some(Gpu {
            name: first.name,
            usage: first.usage,
            temperature: first.temperature,
            backend: GpuBackend::NvidiaSmi(shared),
        })
    }

    fn refresh(&mut self) {
        match &self.backend {
            GpuBackend::Sysfs {
                usage_path,
                temperature_path,
            } => {
                if let Some(usage) = read_number(usage_path) {
                    self.usage = usage as f32;
                }

                self.temperature = temperature_path
                    .as_ref()
                    .and_then(|path| read_number(path))
                    .map(|milli| (milli / 1000.0) as f32);
            }
            GpuBackend::NvidiaSmi(shared) => {
                if let Ok(sample) = shared.0.lock()
                    && let Some(sample) = *sample
                {
                    self.usage = sample.usage;
                    self.temperature = sample.temperature;
                }
            }
        }
    }
}

fn is_drm_card(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn read_number(path: &std::path::Path) -> Option<f64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn gpu_temperature_path(device: &std::path::Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(device.join("hwmon")).ok()?.flatten() {
        let temperature = entry.path().join("temp1_input");
        if temperature.exists() {
            return Some(temperature);
        }
    }

    None
}

fn gpu_name_from_device(device: &std::path::Path) -> Option<String> {
    let uevent = std::fs::read_to_string(device.join("uevent")).ok()?;
    let pci_id = uevent
        .lines()
        .find_map(|line| line.strip_prefix("PCI_ID="))?;
    let (vendor, device) = pci_id.split_once(':')?;

    let vendor = u16::from_str_radix(vendor.trim(), 16).ok()?;
    let device = u16::from_str_radix(device.trim(), 16).ok()?;

    lookup_pci_name(vendor, device)
}

fn lookup_pci_name(vendor: u16, device: u16) -> Option<String> {
    const PCI_IDS_PATHS: [&str; 3] = [
        "/usr/share/hwdata/pci.ids",
        "/usr/share/misc/pci.ids",
        "/var/lib/pciutils/pci.ids",
    ];

    for path in PCI_IDS_PATHS {
        if let Ok(contents) = std::fs::read_to_string(path)
            && let Some(name) = parse_pci_ids(&contents, vendor, device)
        {
            return Some(name);
        }
    }

    None
}

fn parse_pci_ids(contents: &str, vendor: u16, device: u16) -> Option<String> {
    let vendor_line = format!("{vendor:04x}  ");
    let device_line = format!("\t{device:04x}  ");
    let mut in_vendor = false;

    for line in contents.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if line.starts_with('\t') {
            if in_vendor && let Some(name) = line.strip_prefix(&device_line) {
                return Some(name.trim().to_string());
            }
        } else {
            in_vendor = line.starts_with(&vendor_line);
        }
    }

    None
}

struct NvidiaQuery {
    name: String,
    usage: f32,
    temperature: Option<f32>,
}

const NVIDIA_POLL_INTERVAL: Duration = Duration::from_secs(1);

fn spawn_nvidia_poller() -> NvidiaShared {
    let shared = NvidiaShared::default();
    let worker = shared.clone();

    std::thread::spawn(move || {
        loop {
            let Some(query) = query_nvidia_smi() else {
                return;
            };

            *worker.0.lock().unwrap() = Some(NvidiaSample {
                usage: query.usage,
                temperature: query.temperature,
            });

            std::thread::sleep(NVIDIA_POLL_INTERVAL);
        }
    });

    shared
}

fn query_nvidia_smi() -> Option<NvidiaQuery> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_nvidia_smi(stdout.lines().next()?)
}

fn parse_nvidia_smi(line: &str) -> Option<NvidiaQuery> {
    let mut fields = line.split(',').map(str::trim);

    let name = fields.next()?;
    if name.is_empty() {
        return None;
    }

    let usage = fields.next()?.parse::<f32>().ok()?;
    let temperature = fields.next().and_then(|value| value.parse::<f32>().ok());

    Some(NvidiaQuery {
        name: name.to_string(),
        usage,
        temperature,
    })
}

fn cpu_temperature(components: &Components) -> Option<f32> {
    let mut best: Option<(u8, f32)> = None;

    for component in components.iter() {
        let Some(temperature) = component.temperature() else {
            continue;
        };
        let Some(score) = cpu_temperature_score(component.label()) else {
            continue;
        };

        if best.is_none_or(|(current, _)| score > current) {
            best = Some((score, temperature));
        }
    }

    best.map(|(_, temperature)| temperature)
}

fn cpu_temperature_score(label: &str) -> Option<u8> {
    let label = label.to_ascii_lowercase();

    if ["package", "tctl", "tdie"]
        .into_iter()
        .any(|needle| label.contains(needle))
    {
        Some(3)
    } else if ["k10temp", "coretemp", "zenpower", "cpu"]
        .into_iter()
        .any(|needle| label.contains(needle))
    {
        Some(2)
    } else {
        None
    }
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

    #[test]
    fn recognizes_drm_card_names() {
        assert!(is_drm_card("card0"));
        assert!(is_drm_card("card12"));
        assert!(!is_drm_card("card"));
        assert!(!is_drm_card("card0-DP-1"));
        assert!(!is_drm_card("renderD128"));
    }

    #[test]
    fn parses_pci_ids_entry() {
        let contents = concat!(
            "# comment\n",
            "1002  Advanced Micro Devices, Inc. [AMD/ATI]\n",
            "\t1638  Cezanne [Radeon Vega Series]\n",
            "\t9999  Some Other Device\n",
            "8086  Intel Corporation\n",
            "\t1234  Intel Device\n",
        );

        assert_eq!(
            parse_pci_ids(contents, 0x1002, 0x1638).as_deref(),
            Some("Cezanne [Radeon Vega Series]")
        );
        assert_eq!(
            parse_pci_ids(contents, 0x8086, 0x1234).as_deref(),
            Some("Intel Device")
        );
        assert_eq!(parse_pci_ids(contents, 0x1002, 0x0000), None);
        assert_eq!(parse_pci_ids(contents, 0x10de, 0x1638), None);
    }

    #[test]
    fn parses_nvidia_smi_output() {
        let query = parse_nvidia_smi("NVIDIA GeForce RTX 3080, 42, 65").unwrap();
        assert_eq!(query.name, "NVIDIA GeForce RTX 3080");
        assert_eq!(query.usage, 42.0);
        assert_eq!(query.temperature, Some(65.0));

        let no_temperature = parse_nvidia_smi("NVIDIA GeForce RTX 3080, 42, [N/A]").unwrap();
        assert_eq!(no_temperature.temperature, None);

        assert!(parse_nvidia_smi("").is_none());
        assert!(parse_nvidia_smi("OnlyName").is_none());
        assert!(parse_nvidia_smi("GPU, not-a-number, 50").is_none());
    }

    #[test]
    fn scores_cpu_temperature_labels() {
        assert_eq!(cpu_temperature_score("k10temp Tctl"), Some(3));
        assert_eq!(cpu_temperature_score("Package id 0"), Some(3));
        assert_eq!(cpu_temperature_score("coretemp Core 0"), Some(2));
        assert_eq!(cpu_temperature_score("zenpower"), Some(2));
        assert_eq!(cpu_temperature_score("amdgpu edge"), None);
        assert_eq!(cpu_temperature_score("nvme Composite"), None);
    }
}
