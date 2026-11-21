# Ghostwriter

![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)
![Build](https://img.shields.io/github/actions/workflow/status/alpercitak/ghostwriter/build.yaml?style=flat-square&label=&color=2ea44f)
![License](https://img.shields.io/badge/MIT-2ea44f?style=flat-square)

Generate fake but realistic-looking terminal activity.

```
ghostwriter --mode build --seed my-app
```

Press `q` or `Esc` to quit.

---

## Install

Download the latest binary for your platform from [releases](../../releases):

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `ghostwriter-aarch64-apple-darwin` |
| macOS (Intel) | `ghostwriter-x86_64-apple-darwin` |
| Linux (x86_64) | `ghostwriter-x86_64-unknown-linux-gnu` |
| Linux (arm64) | `ghostwriter-aarch64-unknown-linux-gnu` |
| Windows (x86_64) | `ghostwriter-x86_64-pc-windows-msvc.exe` |

Each release also includes a `SHA256SUMS` file. To verify a download:

```bash
shasum -a 256 -c SHA256SUMS --ignore-missing
```

On macOS / Linux, make it executable and put it on your `PATH`:

```bash
chmod +x ghostwriter-aarch64-apple-darwin
mv ghostwriter-aarch64-apple-darwin /usr/local/bin/ghostwriter
```

Or build from source:

```bash
cargo install --path .
```

## Usage

```
Options:
  --mode <MODE>    Activity to simulate [default: build]
                   build | deploy | incident | review
  --seed <SEED>    Repo or author name [default: random, e.g. quiet-ledger]
  --speed <SPEED>  Playback speed multiplier, 0.01–100 [default: 1.0]
  --loop           Start a new run after each one finishes, until you quit
```

Same `--seed` always produces the same output, useful for repeatable screen recordings. Without `--seed` every run is different; the seed is shown in the title bar, so you can replay a run you liked with `--seed <name>`.

```bash
ghostwriter --mode incident --seed payments-api
ghostwriter --mode deploy --seed my-app --speed 2.0
ghostwriter --mode review --seed my-app --speed 0.5
ghostwriter --mode build --loop    # runs forever, q to quit
```

With `--loop`, runs use the seeds `<seed>`, `<seed>-1`, `<seed>-2`, … so a looped session is repeatable too.

## Modes

| Mode | Simulates |
|------|-----------|
| `build` | webpack compilation — modules, warnings, asset sizes |
| `deploy` | Kubernetes rollout — image pull, pod scheduling, health checks |
| `incident` | On-call response — alert, triage, mitigation, resolution |
| `review` | CI pipeline — lint, test suite with flaky retries, coverage, approval |

## Build from source

```bash
cargo build --release
# binary at target/release/ghostwriter
```

Requires Rust stable ≥ 1.95. Install via [rustup.rs](https://rustup.rs).

## License
 
MIT