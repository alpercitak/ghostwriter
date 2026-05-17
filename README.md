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

Download the latest binary from [releases](../../releases) for your platform, then:

```bash
# macOS / Linux
chmod +x ghostwriter-x86_64-apple-darwin
mv ghostwriter-x86_64-apple-darwin /usr/local/bin/ghostwriter

# or build from source
cargo install --path .
```

## Usage

```
Options:
  --mode <MODE>    Activity to simulate [default: build]
                   build | deploy | incident | review
  --seed <SEED>    Repo or author name [default: my-app]
  --speed <SPEED>  Playback speed multiplier [default: 1.0]
```

Same `--seed` always produces the same output, useful for repeatable screen recordings.

```bash
ghostwriter --mode incident --seed payments-api
ghostwriter --mode deploy --seed my-app --speed 2.0
ghostwriter --mode review --seed my-app --speed 0.5
```

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