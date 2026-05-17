use std::time::Instant;

use rand::SeedableRng;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{RenderState, Status, Tick, TickResult, hash_seed};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Resolving,  // "resolving dependencies…"
    Compiling,  // module-by-module output
    Optimizing, // "tree-shaking", "minifying"
    Emitting,   // final asset sizes
    Done,
}

impl Phase {
    fn next(self) -> Self {
        match self {
            Phase::Resolving => Phase::Compiling,
            Phase::Compiling => Phase::Optimizing,
            Phase::Optimizing => Phase::Emitting,
            Phase::Emitting => Phase::Done,
            Phase::Done => Phase::Done,
        }
    }
}

pub struct BuildMode {
    rng: ChaCha8Rng,

    log: Vec<Line<'static>>,
    phase: Phase,
    progress: f64,
    status: Status,
    started_at: Instant,

    phase_lines_remaining: u32,

    total_modules: u32,
    compiled_modules: u32,

    wait_ticks: u32,
}

impl BuildMode {
    pub fn new(seed: &str) -> Self {
        let seed_u64 = hash_seed(seed);
        let mut rng = ChaCha8Rng::seed_from_u64(seed_u64);

        let total_modules = rng.random_range(180..420u32);

        let mut s = Self {
            rng,
            log: Vec::new(),
            phase: Phase::Resolving,
            progress: 0.0,
            status: Status::Running,
            started_at: Instant::now(),
            phase_lines_remaining: 3,
            total_modules,
            compiled_modules: 0,
            wait_ticks: 0,
        };

        s.log.push(Line::from(vec![Span::styled(
            format!(
                "[webpack {}.{}.{} compiled with configuration",
                s.rng.random_range(4..6u8),
                s.rng.random_range(0..9u8),
                s.rng.random_range(0..20u8)
            ),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]));
        s.log.push(Line::raw(""));
        s
    }
}

impl Tick for BuildMode {
    async fn tick(&mut self) -> TickResult {
        if self.phase == Phase::Done {
            return TickResult::Done;
        }

        if self.wait_ticks > 0 {
            self.wait_ticks -= 1;
            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
            return TickResult::Continue;
        }

        match self.phase {
            Phase::Resolving => self.tick_resolving(),
            Phase::Compiling => self.tick_compiling(),
            Phase::Optimizing => self.tick_optimizing(),
            Phase::Emitting => self.tick_emitting(),
            Phase::Done => {}
        }

        TickResult::Continue
    }

    fn state(&self) -> RenderState<'_> {
        RenderState {
            title: "webpack build",
            log_lines: &self.log,
            progress: self.progress,
            status: self.status,
            elapsed_secs: self.started_at.elapsed().as_secs(),
        }
    }
}

impl BuildMode {
    fn tick_resolving(&mut self) {
        let lines = [
            "asset ./dist/main.js",
            "asset ./dist/vendors~main.js",
            "  resolve ./node_modules → entry point",
        ];
        let idx = (3 - self.phase_lines_remaining) as usize;
        self.log
            .push(Line::raw(format!("  {}", lines[idx.min(lines.len() - 1)])));
        self.wait_ticks = self.rng.random_range(2..8);

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.log.push(Line::raw(""));
            self.advance_phase();
        }
    }

    fn tick_compiling(&mut self) {
        let module = random_module_path(&mut self.rng);
        let bytes = self.rng.random_range(512..98_000u32);
        self.log.push(Line::from(vec![
            Span::styled(
                format!(
                    "  [{:>3}/{:>3}] ",
                    self.compiled_modules + 1,
                    self.total_modules
                ),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(format_bytes(bytes), Style::default().fg(Color::Cyan)),
            Span::raw(format!(" {module}")),
        ]));

        self.compiled_modules += 1;
        self.progress = self.compiled_modules as f64 / self.total_modules as f64 * 0.7;

        if self.rng.random_bool(0.04) {
            self.log.push(Line::from(vec![
                Span::styled("  ⚠ WARNING ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("in {module}")),
            ]));
            self.log
                .push(Line::raw("    Module not found, but mocked by externals"));
        }

        self.wait_ticks = if self.rng.random_bool(0.05) {
            self.rng.random_range(8..20)
        } else {
            self.rng.random_range(0..3)
        };

        if self.compiled_modules >= self.total_modules {
            self.log.push(Line::raw(""));
            self.advance_phase();
        }
    }

    fn tick_optimizing(&mut self) {
        let steps = [
            "optimizing…",
            "tree shaking…",
            "scope hoisting…",
            "asset optimization…",
            "minifying…",
        ];
        let idx = (5 - self.phase_lines_remaining) as usize;
        self.log.push(Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(Color::Magenta)),
            Span::raw(steps[idx.min(steps.len() - 1)]),
        ]));
        self.progress = 0.7 + (idx as f64 / 5.0) * 0.2;
        self.wait_ticks = self.rng.random_range(12..40);

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.log.push(Line::raw(""));
            self.advance_phase();
        }
    }

    fn tick_emitting(&mut self) {
        let assets: &[(&str, u32, u32)] = &[
            ("./dist/main.js", 280, 890),
            ("./dist/vendors~main.js", 540, 1_800),
            ("./dist/main.css", 42, 180),
            ("./dist/runtime.js", 8, 32),
        ];
        let idx = (4 - self.phase_lines_remaining) as usize;
        if let Some(&(name, lo, hi)) = assets.get(idx) {
            let size = self.rng.random_range(lo..hi);
            self.log.push(Line::from(vec![
                Span::styled("  ✔ ", Style::default().fg(Color::Green)),
                Span::raw(format!("{name:<40} {size:>8} KiB")),
            ]));
        }
        self.progress = 0.9 + (idx as f64 / 4.0) * 0.1;
        self.wait_ticks = self.rng.random_range(4..12);

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            let elapsed = self.started_at.elapsed().as_secs_f64();
            self.log.push(Line::raw(""));
            self.log.push(Line::from(vec![Span::styled(
                format!("webpack compiled successfully in {:.0}ms", elapsed * 1000.0),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
            self.progress = 1.0;
            self.status = Status::Success;
            self.advance_phase();
        }
    }

    fn advance_phase(&mut self) {
        self.phase = self.phase.next();
        self.phase_lines_remaining = match self.phase {
            Phase::Resolving => 3,
            Phase::Compiling => self.total_modules,
            Phase::Optimizing => 5,
            Phase::Emitting => 4,
            Phase::Done => 0,
        };
    }
}

fn format_bytes(b: u32) -> String {
    if b >= 1024 {
        format!("{:>5} KiB", b / 1024)
    } else {
        format!("{b:>5} B  ")
    }
}

static PACKAGES: &[&str] = &[
    "react",
    "lodash",
    "axios",
    "moment",
    "express",
    "webpack",
    "babel",
    "typescript",
    "eslint",
    "prettier",
    "redux",
    "react-router",
    "styled-components",
    "framer-motion",
];
static DIRS: &[&str] = &[
    "components",
    "hooks",
    "utils",
    "pages",
    "lib",
    "store",
    "services",
    "assets",
    "views",
    "helpers",
];
static EXTS: &[&str] = &[".js", ".ts", ".tsx", ".jsx", ".mjs"];

fn random_module_path(rng: &mut ChaCha8Rng) -> String {
    if rng.random_bool(0.3) {
        // node_modules reference
        format!(
            "./node_modules/{}/index{}",
            PACKAGES[rng.random_range(0..PACKAGES.len())],
            EXTS[rng.random_range(0..EXTS.len())]
        )
    } else {
        // Project source file
        format!(
            "./src/{}/{}{}",
            DIRS[rng.random_range(0..DIRS.len())],
            random_name(rng),
            EXTS[rng.random_range(0..EXTS.len())]
        )
    }
}

fn random_name(rng: &mut ChaCha8Rng) -> &'static str {
    static NAMES: &[&str] = &[
        "index",
        "App",
        "Header",
        "Footer",
        "Button",
        "Modal",
        "Card",
        "Table",
        "Form",
        "Input",
        "Select",
        "Dropdown",
        "Sidebar",
        "Layout",
        "Router",
        "api",
        "auth",
        "helpers",
        "types",
        "config",
        "store",
        "reducer",
        "actions",
        "selectors",
        "middleware",
    ];
    NAMES[rng.random_range(0..NAMES.len())]
}
