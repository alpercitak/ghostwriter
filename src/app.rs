use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend};

use crate::{
    Mode,
    modes::{
        Status, TICK, Tick, TickResult, build::BuildMode, deploy::DeployMode,
        incident::IncidentMode, review::ReviewMode, terraform::TerraformMode,
    },
    ui,
};

/// How long the finished screen stays up before the next run in loop mode.
const LINGER_TICKS: u32 = 20;

#[derive(PartialEq)]
enum Outcome {
    Finished,
    Quit,
}

pub struct App {
    mode: Mode,
    seed: String,
    speed: f32,
    looping: bool,
}

impl App {
    pub fn new(mode: Mode, seed: String, speed: f32, looping: bool) -> Self {
        Self {
            mode,
            seed,
            speed,
            looping,
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        // Enters raw mode + alternate screen and installs a panic hook that
        // restores the terminal, so a crash never leaves the shell unusable.
        let mut terminal = ratatui::try_init()?;

        let result = match self.mode {
            Mode::Build => self.run_mode(&mut terminal, BuildMode::new),
            Mode::Deploy => self.run_mode(&mut terminal, DeployMode::new),
            Mode::Incident => self.run_mode(&mut terminal, IncidentMode::new),
            Mode::Review => self.run_mode(&mut terminal, ReviewMode::new),
            Mode::Terraform => self.run_mode(&mut terminal, TerraformMode::new),
        };

        // Restore even if the event loop returned an error.
        ratatui::restore();

        result
    }

    /// Play one run, or with `--loop` keep playing runs until the user quits.
    /// Looped runs use `<seed>-1`, `<seed>-2`, … so a session is reproducible.
    fn run_mode<B, M>(
        &self,
        terminal: &mut Terminal<B>,
        new_mode: fn(&str) -> M,
    ) -> anyhow::Result<()>
    where
        B: Backend,
        M: Tick,
        <B as Backend>::Error: std::error::Error + Send + Sync + 'static,
    {
        // One tick = one step of the simulation; all pacing lives here so
        // `--speed` scales the whole playback uniformly.
        let tick = TICK.div_f32(self.speed);

        for run in 0.. {
            let seed = match run {
                0 => self.seed.clone(),
                n => format!("{}-{n}", self.seed),
            };
            let mut mode = new_mode(&seed);

            if self.play(terminal, &mut mode, &seed, tick)? == Outcome::Quit {
                break;
            }
            self.draw(terminal, &mode, &seed)?;

            if !self.looping {
                while poll_key_press(Duration::from_millis(200))?.is_none() {}
                break;
            }
            if wait_ticks(LINGER_TICKS, tick, is_quit)? {
                break;
            }
        }

        Ok(())
    }

    /// Tick a mode until it is done, redrawing every tick.
    fn play<B, M>(
        &self,
        terminal: &mut Terminal<B>,
        mode: &mut M,
        seed: &str,
        tick: Duration,
    ) -> anyhow::Result<Outcome>
    where
        B: Backend,
        M: Tick,
        <B as Backend>::Error: std::error::Error + Send + Sync + 'static,
    {
        loop {
            self.draw(terminal, mode, seed)?;

            // Once the run has finished, the UI already says "press any key",
            // so any key exits even if the mode still has a few ticks to play.
            // In loop mode only the quit keys stop it.
            let any_key_exits = !self.looping && mode.state().status != Status::Running;
            if wait_ticks(1, tick, |key| any_key_exits || is_quit(key))? {
                return Ok(Outcome::Quit);
            }

            if let TickResult::Done = mode.tick() {
                return Ok(Outcome::Finished);
            }
        }
    }

    fn draw<B, M>(&self, terminal: &mut Terminal<B>, mode: &M, seed: &str) -> anyhow::Result<()>
    where
        B: Backend,
        M: Tick,
        <B as Backend>::Error: std::error::Error + Send + Sync + 'static,
    {
        terminal.draw(|frame| ui::render(frame, mode.state(), seed, self.looping))?;
        Ok(())
    }
}

/// Wait out `ticks` ticks while staying responsive to key presses. Returns
/// true as soon as a key press satisfies `stop`.
fn wait_ticks(
    ticks: u32,
    tick: Duration,
    stop: impl Fn(KeyEvent) -> bool,
) -> std::io::Result<bool> {
    let deadline = Instant::now() + tick * ticks;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        if let Some(key) = poll_key_press(remaining)?
            && stop(key)
        {
            return Ok(true);
        }
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
