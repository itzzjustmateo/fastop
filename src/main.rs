use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

const TICK_RATE: Duration = Duration::from_millis(500);

struct App {
    running: bool,
}

fn main() -> std::io::Result<()> {
    ratatui::run(|terminal| App::new().run(terminal))
}

impl App {
    fn new() -> Self {
        Self { running: true }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let mut last_tick = Instant::now();

        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
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
            _ => {}
        }
    }

    fn on_tick(&mut self) {}

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

        frame.render_widget(Block::bordered().title("Fastop"), header);
        frame.render_widget(Block::bordered().title("CPU"), cpu);
        frame.render_widget(Block::bordered().title("Memory"), memory);
        frame.render_widget(Block::bordered().title("History"), history);
        frame.render_widget(Block::bordered().title("Processes"), processes);
        frame.render_widget(Paragraph::new("q quit"), footer);
    }
}
