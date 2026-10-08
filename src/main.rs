use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

const TICK_RATE: Duration = Duration::from_millis(500);

struct App {
    running: bool,
    ticks: u64,
    key_presses: u64,
}

fn main() -> std::io::Result<()> {
    ratatui::run(|terminal| App::new().run(terminal))
}

impl App {
    fn new() -> Self {
        Self {
            running: true,
            ticks: 0,
            key_presses: 0,
        }
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
        self.key_presses += 1;

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            _ => {}
        }
    }

    fn on_tick(&mut self) {
        self.ticks += 1;
    }

    fn render(&mut self, frame: &mut Frame) {
        let text = format!(
            "FastTop is alive! ticks: #{}, key presses: #{}",
            self.ticks, self.key_presses
        );

        frame.render_widget(Paragraph::new(text), frame.area());
    }
}
