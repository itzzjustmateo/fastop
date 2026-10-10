use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::TableState;
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};
use sysinfo::{
    Components, Disks, MINIMUM_CPU_UPDATE_INTERVAL, Networks, ProcessesToUpdate, System, Users,
};

use crate::cli::LayoutMode;
use crate::gpu::Gpu;
use crate::layout::{PanelKind, compact_panels, header_height, panels_layout};
use crate::network::{NetworkRow, is_network_interface};
use crate::processes::{ProcessRow, SortBy, compare_processes};

/// All application state and the event loop.
pub(crate) struct App {
    pub(crate) running: bool,
    pub(crate) system: System,
    pub(crate) components: Components,
    pub(crate) disks: Disks,
    pub(crate) networks: Networks,
    pub(crate) network_rows: Vec<NetworkRow>,
    pub(crate) network_tick: Instant,
    pub(crate) users: Users,
    pub(crate) host_name: String,
    pub(crate) os_name: String,
    pub(crate) cpu_name: String,
    pub(crate) gpu: Option<Gpu>,
    pub(crate) processes: Vec<ProcessRow>,
    pub(crate) table_state: TableState,
    pub(crate) sort: SortBy,
    pub(crate) tick: Duration,
    pub(crate) layout: LayoutMode,
}

impl App {
    /// Builds the app from the resolved settings and takes a first sample.
    pub(crate) fn new(tick: Duration, layout: LayoutMode) -> Self {
        let mut system = System::new_all();
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();

        let cpu_name = system
            .cpus()
            .first()
            .map_or_else(|| "Unknown CPU".to_string(), |cpu| cpu.brand().to_string());

        let mut app = Self {
            running: true,
            system,
            components: Components::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            network_rows: Vec::new(),
            network_tick: Instant::now(),
            users: Users::new_with_refreshed_list(),
            host_name: System::host_name().unwrap_or_else(|| "unknown".into()),
            os_name: System::long_os_version().unwrap_or_else(|| "unknown OS".into()),
            cpu_name,
            gpu: Gpu::detect(),
            processes: Vec::new(),
            table_state: TableState::default(),
            sort: SortBy::Cpu,
            tick,
            layout,
        };

        app.on_tick();
        app
    }

    pub(crate) fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let mut last_tick = Instant::now();

        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            let timeout = self.tick.saturating_sub(last_tick.elapsed());

            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
            {
                self.handle_key(key);
            }

            if last_tick.elapsed() >= self.tick {
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
                self.sort = self.sort.next();
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
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.users.refresh();

        if let Some(gpu) = &mut self.gpu {
            gpu.refresh();
        }

        self.update_process_list();
        self.update_network();
    }

    fn update_network(&mut self) {
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.network_tick)
            .as_secs_f64()
            .max(0.05);
        self.network_tick = now;

        let mut rows: Vec<NetworkRow> = self
            .networks
            .list()
            .iter()
            .filter(|(name, _)| is_network_interface(name))
            .map(|(name, data)| NetworkRow {
                name: name.clone(),
                received: data.received() as f64 / elapsed,
                transmitted: data.transmitted() as f64 / elapsed,
            })
            .collect();

        rows.sort_by(|a, b| {
            (b.received + b.transmitted)
                .total_cmp(&(a.received + a.transmitted))
                .then_with(|| a.name.cmp(&b.name))
        });

        self.network_rows = rows;
    }

    fn update_process_list(&mut self) {
        let users = &self.users;

        let mut processes: Vec<ProcessRow> = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let user = process
                    .user_id()
                    .and_then(|uid| users.get_user_by_id(uid))
                    .map_or_else(|| "?".to_string(), |user| user.name().to_string());

                ProcessRow {
                    pid: pid.as_u32(),
                    user,
                    name: process.name().to_string_lossy().into_owned(),
                    cpu: process.cpu_usage(),
                    memory: process.memory(),
                }
            })
            .collect();

        processes.sort_by(|a, b| compare_processes(self.sort, a, b));

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
        let area = frame.area();

        let header_height = header_height(area);
        let footer_height = u16::from(area.height >= 4);
        let (grid, panels_height) = panels_layout(area, self.layout);

        let [header, panels, processes, footer] = Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(panels_height),
            Constraint::Fill(1),
            Constraint::Length(footer_height),
        ])
        .areas(area);

        if panels_height > 0 {
            if grid {
                let [device_rows, network_row] =
                    Layout::vertical([Constraint::Length(18), Constraint::Length(6)]).areas(panels);

                let [cpu_row, device_row] =
                    Layout::vertical([Constraint::Ratio(1, 2); 2]).areas(device_rows);

                let [cpu, memory] = Layout::horizontal([Constraint::Ratio(1, 2); 2]).areas(cpu_row);
                let [gpu, disks] =
                    Layout::horizontal([Constraint::Ratio(1, 2); 2]).areas(device_row);

                self.render_cpu(frame, cpu);
                self.render_memory(frame, memory);
                self.render_gpu(frame, gpu);
                self.render_disks(frame, disks);
                self.render_network(frame, network_row);
            } else {
                let kinds = compact_panels(area.width);
                let constraints = vec![Constraint::Ratio(1, kinds.len() as u32); kinds.len()];
                let chunks = Layout::horizontal(constraints).split(panels);

                for (kind, chunk) in kinds.iter().zip(chunks.iter()) {
                    self.render_panel(frame, *kind, *chunk);
                }
            }
        }

        self.render_header(frame, header);
        self.render_processes(frame, processes);
        self.render_footer(frame, footer);
    }

    fn render_panel(&self, frame: &mut Frame, kind: PanelKind, area: Rect) {
        match kind {
            PanelKind::Cpu => self.render_cpu(frame, area),
            PanelKind::Memory => self.render_memory(frame, area),
            PanelKind::Gpu => self.render_gpu(frame, area),
            PanelKind::Disks => self.render_disks(frame, area),
            PanelKind::Network => self.render_network(frame, area),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(Duration::from_millis(500), LayoutMode::Auto)
    }

    fn grid_app() -> App {
        App::new(Duration::from_millis(500), LayoutMode::Grid)
    }

    #[test]
    fn renders_without_panicking() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = test_app();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        assert_eq!(terminal.backend().buffer().area.width, 120);
    }

    #[test]
    fn renders_forced_grid_on_short_terminals() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = grid_app();

        for height in [28, 30, 34] {
            let mut terminal = Terminal::new(TestBackend::new(120, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            assert_eq!(terminal.backend().buffer().area.height, height);
        }
    }

    #[test]
    fn renders_across_terminal_sizes() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = test_app();
        let sizes = [
            (12, 4),
            (24, 8),
            (30, 10),
            (40, 12),
            (60, 20),
            (80, 24),
            (100, 30),
            (160, 48),
        ];

        for (width, height) in sizes {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            assert_eq!(terminal.backend().buffer().area.width, width);
        }
    }
}
