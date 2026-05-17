mod app;
mod modes;
mod ui;

use clap::{Parser, ValueEnum};
use app::App;

/// ghostwriter — generate realistic-looking fake terminal activity
#[derive(Parser)]
#[command(name = "ghostwriter", version, about, long_about = None)]
struct Cli {
    /// The activity mode to simulate
    #[arg(long, default_value = "build")]
    mode: Mode,

    /// Optional seed string (e.g. repo name) for consistent output
    #[arg(long, default_value = "ghost")]
    seed: String,
}

#[derive(Clone, ValueEnum)]
pub enum Mode {
    Build,
    Deploy,
    Incident
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut app = App::new(cli.mode, cli.seed);
    app.run().await
}
