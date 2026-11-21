use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{Sim, Status, Tick};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Resolving,  // "resolving dependencies…"
    Compiling,  // module-by-module output
    Optimizing, // "tree-shaking", "minifying"
    Emitting,   // final asset sizes
    Done,
}

pub struct BuildMode {
    sim: Sim,
    phase: Phase,

    total_modules: u32,
    compiled_modules: u32,
}

impl BuildMode {
    pub fn new(seed: &str) -> Self {
        let mut sim = Sim::new(seed);

        let total_modules = sim.rng.random_range(180..420u32);

        let mut s = Self {
            sim,
            phase: Phase::Resolving,
            total_modules,
            compiled_modules: 0,
        };

        s.sim.log.push(Line::from(vec![Span::styled(
            format!(
                "webpack {}.{}.{} — building in production mode",
                s.sim.rng.random_range(4..6u8),
                s.sim.rng.random_range(0..9u8),
                s.sim.rng.random_range(0..20u8)
            ),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]));
        s.sim.log.push(Line::raw(""));
        s
    }
}

impl Tick for BuildMode {
    const TITLE: &'static str = "webpack build";
    const SUCCESS_MESSAGE: &'static str = "build succeeded";

    fn sim(&self) -> &Sim {
        &self.sim
    }

    fn sim_mut(&mut self) -> &mut Sim {
        &mut self.sim
    }

    fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }

    fn step(&mut self) {
        match self.phase {
            Phase::Resolving => self.tick_resolving(),
            Phase::Compiling => self.tick_compiling(),
            Phase::Optimizing => self.tick_optimizing(),
            Phase::Emitting => self.tick_emitting(),
            Phase::Done => {}
        }
    }
}

impl BuildMode {
    fn tick_resolving(&mut self) {
        let lines = [
            "resolving entry ./src/index.tsx",
            "resolving ./node_modules",
            "resolved dependency graph",
        ];
        let idx = self.sim.step as usize;
        self.sim
            .log
            .push(Line::raw(format!("  {}", lines[idx.min(lines.len() - 1)])));
        self.sim.wait_ticks = self.sim.rng.random_range(2..8);

        if self.sim.next_step(3) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Compiling;
        }
    }

    fn tick_compiling(&mut self) {
        let module = random_module_path(&mut self.sim.rng);
        let bytes = self.sim.rng.random_range(512..98_000u32);
        self.sim.log.push(Line::from(vec![
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
        self.sim.progress = self.compiled_modules as f64 / self.total_modules as f64 * 0.7;

        if self.sim.rng.random_bool(0.04) {
            self.sim.log.push(Line::from(vec![
                Span::styled("  ⚠ WARNING ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("in {module}")),
            ]));
            self.sim
                .log
                .push(Line::raw("    Module not found, but mocked by externals"));
        }

        self.sim.wait_ticks = if self.sim.rng.random_bool(0.05) {
            self.sim.rng.random_range(8..20)
        } else {
            self.sim.rng.random_range(0..3)
        };

        if self.compiled_modules >= self.total_modules {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Optimizing;
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
        let idx = self.sim.step as usize;
        self.sim.log.push(Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(Color::Magenta)),
            Span::raw(steps[idx.min(steps.len() - 1)]),
        ]));
        self.sim.progress = 0.7 + (idx as f64 / 5.0) * 0.2;
        self.sim.wait_ticks = self.sim.rng.random_range(12..40);

        if self.sim.next_step(5) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Emitting;
        }
    }

    fn tick_emitting(&mut self) {
        let assets: &[(&str, u32, u32)] = &[
            ("./dist/main.js", 280, 890),
            ("./dist/vendors~main.js", 540, 1_800),
            ("./dist/main.css", 42, 180),
            ("./dist/runtime.js", 8, 32),
        ];
        let idx = self.sim.step as usize;
        if let Some(&(name, lo, hi)) = assets.get(idx) {
            let size = self.sim.rng.random_range(lo..hi);
            self.sim.log.push(Line::from(vec![
                Span::styled("  ✔ ", Style::default().fg(Color::Green)),
                Span::raw(format!("{name:<40} {size:>8} KiB")),
            ]));
        }
        self.sim.progress = 0.9 + (idx as f64 / 4.0) * 0.1;
        self.sim.wait_ticks = self.sim.rng.random_range(4..12);

        if self.sim.next_step(4) {
            let elapsed = self.sim.elapsed().as_secs_f64();
            self.sim.log.push(Line::raw(""));
            self.sim.log.push(Line::from(vec![Span::styled(
                format!("webpack compiled successfully in {:.0}ms", elapsed * 1000.0),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
            self.sim.progress = 1.0;
            self.sim.status = Status::Success;
            self.phase = Phase::Done;
        }
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
