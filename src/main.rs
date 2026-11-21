mod app;
mod modes;
mod ui;

use app::App;
use clap::{Parser, ValueEnum};

/// ghostwriter — generate realistic-looking fake terminal activity
#[derive(Parser)]
#[command(name = "ghostwriter", version, about, long_about = None)]
struct Cli {
    /// The activity mode to simulate
    #[arg(long, default_value = "build")]
    mode: Mode,

    /// Optional seed string (e.g. repo name) for consistent output
    #[arg(long, default_value = "my-app")]
    seed: String,

    /// Playback speed multiplier (0.5 = half speed, 2.0 = double)
    #[arg(long, default_value = "1.0")]
    speed: f32,
}

#[derive(Clone, ValueEnum)]
pub enum Mode {
    Build,
    Deploy,
    Incident,
    Review,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut app = App::new(cli.mode, cli.seed, cli.speed);
    app.run().await
}
