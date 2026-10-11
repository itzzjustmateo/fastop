use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::TableState;
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

use crate::cli::LayoutMode;
use crate::collect::{CollectOptions, Collector};
use crate::layout::{compact_panels, header_height, panels_layout};
use crate::processes::SortBy;

/// Application state: a [`Collector`] plus the UI state and the event loop.
///
/// The collector owns every data source; the panels read from it through
/// accessors, so adding a panel never touches the collection logic.
pub(crate) struct App {
    pub(crate) running: bool,
    pub(crate) collector: Collector,
    pub(crate) table_state: TableState,
    pub(crate) sort: SortBy,
    pub(crate) tick: Duration,
    pub(crate) layout: LayoutMode,
}

impl App {
    /// Builds the app from resolved settings and takes a first sample.
    pub(crate) fn new(
        tick: Duration,
        layout: LayoutMode,
        sort: SortBy,
        options: CollectOptions,
    ) -> Self {
        let mut app = Self {
            running: true,
            collector: Collector::new(options),
            table_state: TableState::default(),
            sort,
            tick,
            layout,
        };
        app.collector.resort(sort);
        app.clamp_process_selection();
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
                self.collector.resort(self.sort);
            }
            _ => {}
        }
    }

    fn select_next(&mut self) {
        let count = self.collector.process_rows().len();
        let next = match self.table_state.selected() {
            Some(index) if index + 1 < count => index + 1,
            _ => 0,
        };

        if count > 0 {
            self.table_state.select(Some(next));
        }
    }

    fn select_previous(&mut self) {
        let count = self.collector.process_rows().len();
        let previous = match self.table_state.selected() {
            Some(0) | None => count.saturating_sub(1),
            Some(index) => index - 1,
        };

        if count > 0 {
            self.table_state.select(Some(previous));
        }
    }

    fn on_tick(&mut self) {
        self.collector.refresh(self.sort);
        self.clamp_process_selection();
    }

    /// Keeps the table selection within the new process row count.
    fn clamp_process_selection(&mut self) {
        let count = self.collector.process_rows().len();
        let selected = self
            .table_state
            .selected()
            .unwrap_or(0)
            .min(count.saturating_sub(1));

        self.table_state.select((count != 0).then_some(selected));
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
                let [cpu_row, device_row, network_row] = Layout::vertical([
                    Constraint::Length(9),
                    Constraint::Length(7),
                    Constraint::Length(6),
                ])
                .areas(panels);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(
            Duration::from_millis(500),
            LayoutMode::Auto,
            SortBy::Cpu,
            CollectOptions::default(),
        )
    }

    fn grid_app() -> App {
        App::new(
            Duration::from_millis(500),
            LayoutMode::Grid,
            SortBy::Cpu,
            CollectOptions::default(),
        )
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

    #[test]
    fn clamps_selection_to_the_process_count() {
        let mut app = test_app();
        app.table_state.select(Some(usize::MAX));

        app.clamp_process_selection();

        if let Some(selected) = app.table_state.selected() {
            assert!(selected < app.collector.process_rows().len());
        }
    }
}
