mod app;
mod modes;
mod ui;

use app::App;
use clap::{Parser, ValueEnum};
use rand::seq::IndexedRandom;

/// ghostwriter — generate realistic-looking fake terminal activity
#[derive(Parser)]
#[command(name = "ghostwriter", version, about, long_about = None)]
struct Cli {
    /// The activity mode to simulate
    #[arg(long, default_value = "build")]
    mode: Mode,

    /// Repo or service name; the same seed always gives the same output.
    /// Defaults to a random name, shown in the title bar
    #[arg(long)]
    seed: Option<String>,

    /// Playback speed multiplier (0.5 = half speed, 2.0 = double)
    #[arg(long, default_value = "1.0", value_parser = parse_speed)]
    speed: f32,

    /// Keep playing: start the next run (seed `<seed>-1`, `<seed>-2`, …)
    /// after each one finishes
    #[arg(long = "loop")]
    looping: bool,
}

#[derive(Clone, ValueEnum)]
pub enum Mode {
    Build,
    Deploy,
    Incident,
    Review,
}

const SPEED_RANGE: std::ops::RangeInclusive<f32> = 0.01..=100.0;

fn parse_speed(s: &str) -> Result<f32, String> {
    let speed: f32 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if SPEED_RANGE.contains(&speed) {
        Ok(speed)
    } else {
        Err(format!(
            "must be between {} and {}",
            SPEED_RANGE.start(),
            SPEED_RANGE.end()
        ))
    }
}

/// A random project-style name such as `quiet-ledger`.
fn random_seed() -> String {
    const ADJECTIVES: &[&str] = &[
        "amber", "brave", "calm", "crimson", "daring", "eager", "fuzzy", "golden", "hidden",
        "lucky", "mellow", "nimble", "quiet", "rapid", "silent", "steady", "swift", "velvet",
    ];
    const NOUNS: &[&str] = &[
        "atlas", "beacon", "comet", "falcon", "harbor", "ledger", "lantern", "nebula", "orbit",
        "pixel", "quartz", "relay", "summit", "tundra", "vortex", "willow",
    ];
    let mut rng = rand::rng();
    format!(
        "{}-{}",
        ADJECTIVES.choose(&mut rng).expect("non-empty"),
        NOUNS.choose(&mut rng).expect("non-empty"),
    )
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let seed = cli.seed.unwrap_or_else(random_seed);
    let mut app = App::new(cli.mode, seed, cli.speed, cli.looping);
    app.run()
}
