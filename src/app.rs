use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::Backend};

use crate::{
    Mode,
    modes::{
        Tick, TickResult, build::BuildMode, deploy::DeployMode, incident::IncidentMode,
        review::ReviewMode,
    },
    ui,
};

const BASE_TICK: Duration = Duration::from_millis(100);

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
        // Enters raw mode + alternate screen and installs a panic hook that
        // restores the terminal, so a crash never leaves the shell unusable.
        let mut terminal = ratatui::try_init()?;

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

        // Restore even if the event loop returned an error.
        ratatui::restore();

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
        // One tick = one step of the simulation; all pacing lives here so
        // `--speed` scales the whole playback uniformly.
        let tick = BASE_TICK.div_f32(speed.max(0.1));

        loop {
            terminal.draw(|frame| {
                ui::render(frame, mode.state());
            })?;

            // Wait out the tick while staying responsive to key presses.
            let deadline = Instant::now() + tick;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                if event::poll(remaining)?
                    && let Event::Key(key) = event::read()?
                    && is_quit(key)
                {
                    return Ok(());
                }
            }

            match mode.tick().await {
                TickResult::Continue => {}
                TickResult::Done => {
                    terminal.draw(|frame| ui::render(frame, mode.state()))?;
                    loop {
                        if event::poll(Duration::from_millis(200))?
                            && let Event::Key(_) = event::read()?
                        {
                            break;
                        }
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}

/// `q`, `Esc`, or Ctrl+C. Raw mode delivers Ctrl+C as a key event rather
/// than SIGINT, so it has to be handled explicitly.
fn is_quit(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}
