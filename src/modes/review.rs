use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{Sim, Status, Tick};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Linting,   // eslint / clippy style output
    Testing,   // test suite, pass/fail per case, flaky retry
    Coverage,  // coverage report per file
    Approving, // reviewer approval gate
    Done,
}

pub struct ReviewMode {
    sim: Sim,
    phase: Phase,

    author: String,
    branch: String,
    pr_number: u32,

    // test tracking
    total_tests: u32,
    passed_tests: u32,

    // flaky retry state
    retrying: bool,
    test_header_emitted: bool,
}

impl ReviewMode {
    pub fn new(seed: &str) -> Self {
        let mut sim = Sim::new(seed);

        let pr_number = sim.rng.random_range(100..9999u32);
        let branch = format!(
            "{}/{}",
            BRANCH_PREFIXES[sim.rng.random_range(0..BRANCH_PREFIXES.len())],
            seed,
        );
        let total_tests = sim.rng.random_range(24..80u32);

        let mut s = Self {
            sim,
            phase: Phase::Linting,
            author: seed.to_string(),
            branch,
            pr_number,
            total_tests,
            passed_tests: 0,
            retrying: false,
            test_header_emitted: false,
        };

        s.sim.log.push(Line::from(vec![
            Span::styled("  PR #", Style::default().fg(Color::DarkGray)),
            Span::styled(
                s.pr_number.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::raw(s.branch.clone()),
            Span::styled(
                format!("  by {}", s.author),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        s.sim.log.push(Line::raw(""));
        s
    }
}

impl Tick for ReviewMode {
    const TITLE: &'static str = "code review / CI";
    const SUCCESS_MESSAGE: &'static str = "PR approved";

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
            Phase::Linting => self.tick_linting(),
            Phase::Testing => self.tick_testing(),
            Phase::Coverage => self.tick_coverage(),
            Phase::Approving => self.tick_approving(),
            Phase::Done => {}
        }
    }
}

impl ReviewMode {
    fn tick_linting(&mut self) {
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  ▶ lint    ", Style::default().fg(Color::Magenta)),
                    Span::raw("running eslint…"),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(10..20);
                self.sim.progress = 0.04;
            }
            1 | 2 => {
                // occasional lint warning — not an error, just noise
                if self.sim.rng.random_bool(0.6) {
                    let file = random_src_file(&mut self.sim.rng);
                    let line = self.sim.rng.random_range(10..300u32);
                    let rule = LINT_RULES[self.sim.rng.random_range(0..LINT_RULES.len())];
                    self.sim.log.push(Line::from(vec![
                        Span::styled(
                            format!("  warn  {file}:{line} "),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(rule, Style::default().fg(Color::DarkGray)),
                    ]));
                } else {
                    self.sim.log.push(Line::from(vec![
                        Span::styled("  ok    ", Style::default().fg(Color::Green)),
                        Span::raw(random_src_file(&mut self.sim.rng)),
                    ]));
                }
                self.sim.wait_ticks = self.sim.rng.random_range(3..9);
                self.sim.progress = 0.08;
            }
            3 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  ✔ lint    ", Style::default().fg(Color::Green)),
                    Span::raw("no errors  "),
                    Span::styled(
                        format!("{} warnings", self.sim.rng.random_range(0..6u32)),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(4..8);
                self.sim.progress = 0.14;
            }
            _ => {
                self.sim.log.push(Line::raw(""));
                self.sim.wait_ticks = self.sim.rng.random_range(2..5);
            }
        }

        if self.sim.next_step(5) {
            self.phase = Phase::Testing;
        }
    }

    fn tick_testing(&mut self) {
        if !self.test_header_emitted {
            self.test_header_emitted = true;
            self.sim.log.push(Line::from(vec![
                Span::styled("  ▶ test    ", Style::default().fg(Color::Magenta)),
                Span::raw(format!("running {} tests…", self.total_tests)),
            ]));
            self.sim.wait_ticks = self.sim.rng.random_range(4..10);
            return;
        }

        let done = self.passed_tests;

        // flaky test: first attempt fails, retry succeeds
        let flaky = !self.retrying && self.sim.rng.random_bool(0.06);
        let fail = flaky || self.sim.rng.random_bool(0.02);

        let test_name = random_test_name(&mut self.sim.rng);

        if fail && !self.retrying {
            self.sim.log.push(Line::from(vec![
                Span::styled("  ✖ FAIL   ", Style::default().fg(Color::Red)),
                Span::raw(test_name.clone()),
                Span::styled("  (retrying…)", Style::default().fg(Color::Yellow)),
            ]));
            self.retrying = true;
            self.sim.wait_ticks = self.sim.rng.random_range(12..25);
        } else if self.retrying {
            // retry always passes (keeps the mood optimistic)
            self.sim.log.push(Line::from(vec![
                Span::styled("  ✔ RETRY  ", Style::default().fg(Color::Yellow)),
                Span::raw(test_name),
            ]));
            self.retrying = false;
            self.passed_tests += 1;
            self.sim.wait_ticks = self.sim.rng.random_range(2..6);
        } else {
            self.sim.log.push(Line::from(vec![
                Span::styled("  ✔ PASS   ", Style::default().fg(Color::Green)),
                Span::raw(test_name),
            ]));
            self.passed_tests += 1;
            self.sim.wait_ticks = self.sim.rng.random_range(0..4);
        }

        self.sim.progress = 0.14 + (done as f64 / self.total_tests as f64) * 0.40;

        if self.passed_tests >= self.total_tests {
            self.sim.log.push(Line::raw(""));
            self.sim.log.push(Line::from(vec![
                Span::styled("  ✔ tests  ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("{} passed", self.passed_tests),
                    Style::default().fg(Color::Green),
                ),
                Span::styled("  0 failed", Style::default().fg(Color::DarkGray)),
            ]));
            self.sim.log.push(Line::raw(""));
            self.sim.progress = 0.55;
            self.phase = Phase::Coverage;
        }
    }

    fn tick_coverage(&mut self) {
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  ▶ cover  ", Style::default().fg(Color::Magenta)),
                    Span::raw("measuring coverage…"),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(8..18);
                self.sim.progress = 0.60;
            }
            1..=3 => {
                let file = random_src_file(&mut self.sim.rng);
                let pct = self.sim.rng.random_range(62..100u32);
                let color = if pct >= 80 {
                    Color::Green
                } else {
                    Color::Yellow
                };
                self.sim.log.push(Line::from(vec![
                    Span::styled(
                        format!("  {file:<38} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("{pct:>3}%"), Style::default().fg(color)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(3..8);
                self.sim.progress = 0.63 + ((self.sim.step - 1) as f64 / 5.0) * 0.12;
            }
            _ => {
                let total_pct = self.sim.rng.random_range(74..93u32);
                self.sim.log.push(Line::from(vec![
                    Span::styled(
                        "  total                                 ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{total_pct:>3}%"),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                self.sim.log.push(Line::raw(""));
                self.sim.wait_ticks = self.sim.rng.random_range(4..8);
                self.sim.progress = 0.75;
            }
        }

        if self.sim.next_step(5) {
            self.phase = Phase::Approving;
        }
    }

    fn tick_approving(&mut self) {
        let pr = self.pr_number;
        match self.sim.step {
            0 => {
                let reviewer = REVIEWERS[self.sim.rng.random_range(0..REVIEWERS.len())];
                self.sim.log.push(Line::from(vec![
                    Span::styled("  review   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("@{reviewer}"), Style::default().fg(Color::Cyan)),
                    Span::styled(" requested", Style::default().fg(Color::DarkGray)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(20..45);
                self.sim.progress = 0.80;
            }
            1 => {
                let reviewer = REVIEWERS[self.sim.rng.random_range(0..REVIEWERS.len())];
                let comment = REVIEW_COMMENTS[self.sim.rng.random_range(0..REVIEW_COMMENTS.len())];
                self.sim.log.push(Line::from(vec![
                    Span::styled(format!("  @{reviewer}  "), Style::default().fg(Color::Cyan)),
                    Span::styled(comment, Style::default().fg(Color::DarkGray)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(10..22);
                self.sim.progress = 0.88;
            }
            2 => {
                let reviewer = REVIEWERS[self.sim.rng.random_range(0..REVIEWERS.len())];
                self.sim.log.push(Line::from(vec![
                    Span::styled("  ✔ approved  ", Style::default().fg(Color::Green)),
                    Span::styled(format!("@{reviewer}"), Style::default().fg(Color::Cyan)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(6..14);
                self.sim.progress = 0.95;
            }
            _ => {
                self.sim.log.push(Line::raw(""));
                self.sim.log.push(Line::from(vec![Span::styled(
                    format!("  PR #{pr} merged  ✔  all checks passed"),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
                self.sim.progress = 1.0;
                self.sim.status = Status::Success;
                self.sim.wait_ticks = 0;
            }
        }

        if self.sim.next_step(4) {
            self.phase = Phase::Done;
        }
    }
}

// ── Generators ────────────────────────────────────────────────────────────────

fn random_src_file(rng: &mut ChaCha8Rng) -> String {
    static DIRS: &[&str] = &["components", "hooks", "utils", "services", "lib", "pages"];
    static FILES: &[&str] = &[
        "index",
        "api",
        "auth",
        "helpers",
        "types",
        "config",
        "store",
        "reducer",
        "middleware",
        "router",
    ];
    static EXTS: &[&str] = &[".ts", ".tsx", ".js"];
    format!(
        "src/{}/{}{}",
        DIRS[rng.random_range(0..DIRS.len())],
        FILES[rng.random_range(0..FILES.len())],
        EXTS[rng.random_range(0..EXTS.len())],
    )
}

fn random_test_name(rng: &mut ChaCha8Rng) -> String {
    static SUBJECTS: &[&str] = &[
        "AuthService",
        "UserStore",
        "ApiClient",
        "PaymentFlow",
        "SessionManager",
        "RouteGuard",
        "DataMapper",
        "EventBus",
    ];
    static PREDICATES: &[&str] = &[
        "should return 401 on expired token",
        "handles concurrent requests correctly",
        "retries on network failure",
        "validates required fields",
        "emits event on state change",
        "resolves with empty array when no results",
        "rejects invalid input",
        "matches snapshot",
    ];
    format!(
        "{} > {}",
        SUBJECTS[rng.random_range(0..SUBJECTS.len())],
        PREDICATES[rng.random_range(0..PREDICATES.len())],
    )
}

static BRANCH_PREFIXES: &[&str] = &["feat", "fix", "chore", "refactor", "perf"];
static LINT_RULES: &[&str] = &[
    "no-unused-vars",
    "@typescript-eslint/no-explicit-any",
    "react-hooks/exhaustive-deps",
    "no-console",
    "prefer-const",
];
static REVIEWERS: &[&str] = &["sarah_k", "mvanderberg", "tpham", "alex_j", "rbauer"];
static REVIEW_COMMENTS: &[&str] = &[
    "looks good, minor nit on error handling",
    "LGTM — left one optional suggestion",
    "nice refactor, approved",
    "clean solution, left a question inline",
];
