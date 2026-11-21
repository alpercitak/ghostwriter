use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend};

use crate::{
    Mode,
    modes::{
        Status, TICK, Tick, TickResult, build::BuildMode, deploy::DeployMode,
        incident::IncidentMode, review::ReviewMode,
    },
    ui,
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

    pub fn run(&mut self) -> anyhow::Result<()> {
        // Enters raw mode + alternate screen and installs a panic hook that
        // restores the terminal, so a crash never leaves the shell unusable.
        let mut terminal = ratatui::try_init()?;

        let result = match self.mode {
            Mode::Build => {
                let mut mode = BuildMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed)
            }
            Mode::Deploy => {
                let mut mode = DeployMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed)
            }
            Mode::Incident => {
                let mut mode = IncidentMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed)
            }
            Mode::Review => {
                let mut mode = ReviewMode::new(&self.seed);
                self.event_loop(&mut terminal, &mut mode, self.speed)
            }
        };

        // Restore even if the event loop returned an error.
        ratatui::restore();

        result
    }

    fn event_loop<B, M>(
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
        let tick = TICK.div_f32(speed.max(0.1));

        loop {
            terminal.draw(|frame| {
                ui::render(frame, mode.state());
            })?;

            // Wait out the tick while staying responsive to key presses. Once
            // the run has finished, the UI already says "press any key", so
            // any key exits even if the mode still has a few ticks to play.
            let finished = mode.state().status != Status::Running;
            let deadline = Instant::now() + tick;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                if let Some(key) = poll_key_press(remaining)?
                    && (finished || is_quit(key))
                {
                    return Ok(());
                }
            }

            match mode.tick() {
                TickResult::Continue => {}
                TickResult::Done => {
                    terminal.draw(|frame| ui::render(frame, mode.state()))?;
                    loop {
                        if poll_key_press(Duration::from_millis(200))?.is_some() {
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

/// Wait up to `timeout` for an event and return it if it is a key press.
/// Windows also reports key releases (and repeats); only presses count.
fn poll_key_press(timeout: Duration) -> std::io::Result<Option<KeyEvent>> {
    if event::poll(timeout)?
        && let Event::Key(key) = event::read()?
        && key.kind == KeyEventKind::Press
    {
        return Ok(Some(key));
    }
    Ok(None)
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
