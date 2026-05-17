use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, backend::CrosstermBackend, Terminal};

use crate::{
    modes::{build::BuildMode, deploy::DeployMode, incident::IncidentMode, review::ReviewMode, Tick, TickResult},
    ui,
    Mode,
};

pub struct App {
    mode: Mode,
    seed: String,
    speed: f32,
}

impl App {
    pub fn new(mode: Mode, seed: String, speed: f32) -> Self {
        Self { mode, seed, speed }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = match self.mode {
            Mode::Build => {
                let mut mode = BuildMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed).await
            }
            Mode::Deploy => {
                let mut mode = DeployMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed).await
            }
            Mode::Incident => {
                let mut mode = IncidentMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed).await
            }
            Mode::Review => {
                let mut mode = ReviewMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed).await
            }
        };

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn event_loop<B, M>(
        &self,
        terminal: &mut Terminal<B>,
        mode: &mut M,
        speed: f32,
    ) -> anyhow::Result<()>
    where
        B: Backend,
        M: Tick,
        <B as Backend>::Error: std::error::Error + Send + Sync + 'static,
    {
        loop {
            terminal.draw(|frame| {
                ui::render(frame, mode.state());
            })?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                        break;
                    }
                }
            }

            match mode.tick().await {
                TickResult::Continue => {
                    let delay = (50.0 / speed.max(0.1)) as u64;
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                TickResult::Done => {
                    terminal.draw(|frame| ui::render(frame, mode.state()))?;
                    loop {
                        if event::poll(Duration::from_millis(200))? {
                            if let Event::Key(_) = event::read()? {
                                break;
                            }
                        }
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
