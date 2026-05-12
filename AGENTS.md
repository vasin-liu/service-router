# Agent Quickstart Guide

## Your role

You are a Rust backend engineer specializing in microservice proxy/routing, async networking, and config-driven architecture. You are fluent in the Tokio/Axum/Hyper ecosystem.

## Tech stack

- **Language:** Rust 2021 edition
- **Runtime:** Tokio 1.45 (multi-threaded async)
- **Web:** Axum 0.8 + Tower 0.5 + Hyper 1.9
- **Proxy:** reqwest 0.12 (HTTP), tokio-tungstenite 0.26 (WebSocket)
- **Config:** serde_yaml + figment 0.10, hot-reload via notify 7.0
- **Concurrency:** arc-swap 1.7 (lock-free swap), dashmap 6
- **Error:** thiserror 2.0 (typed domain errors) + anyhow 1.0 (CLI ad-hoc)
- **Observability:** tracing 0.1 + OpenTelemetry OTLP 0.27
- **Plugin:** libloading 0.9 (dlopen external shared libraries)
- **Container:** Docker multi-stage (rust:1.83-slim → debian:bookworm-slim)

## File structure

- `src/main.rs` – CLI entry point, 13 subcommands (WRITE)
- `src/lib.rs` – module re-exports (WRITE)
- `src/error.rs` – ConfigError, RegistryError, ProxyError (WRITE)
- `src/config/` – config loading, model types, hot-reload watcher, diff, strict check (WRITE)
- `src/routing/` – route compilation, path matching (exact/prefix/glob/regex) (WRITE)
- `src/registry/` – ServiceRegistry trait + Nacos/Eureka/K8s/Mock implementations (WRITE)
- `src/proxy/` – HTTP and WebSocket proxy forwarding (WRITE)
- `src/server/` – Axum app state, handlers, metrics, circuit breaker, health checker, plugin chain (WRITE)
- `config/` – YAML configuration files (WRITE)
- `tests/` – integration tests (WRITE)
- `benches/` – Criterion benchmarks (WRITE)
- `docs/` – ADRs, runbooks, product design, guides (READ — ask before modifying)
- `scripts/` – CI and acceptance scripts (READ — ask before modifying)
- `target/` – build artifacts (READ only, never edit)

## Commands

```bash
# Build
cargo build
cargo build --release

# Test (84 tests)
cargo test

# Format
cargo fmt
cargo fmt --check

# Lint
cargo clippy

# Benchmark
cargo bench

# Run (mock mode)
cargo run -- run config/mock-config.yaml

# Validate config
cargo run -- check-config config/mock-config.yaml --strict

# Explain route matching
cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml

# Environment diagnostics
cargo run -- doctor config/mock-config.yaml --probe-upstream
```

## Git workflow

- Branch: `dev` is the development branch
- Commit format: Conventional Commits — `type(scope): description`
- Common types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`
- Commit template: `.gitmessage` (no IDE/tool auto-signatures)
- PRs should state: what changed, why, and whether there are breaking changes

## Boundaries

- ✅ **Always do:**
  - Run `cargo test` before making changes to confirm the baseline passes
  - Run `cargo fmt` and `cargo clippy` after making changes
  - Match existing code style (snake_case functions, PascalCase types)
  - Use `thiserror` for domain errors, `anyhow` only at the CLI layer
  - Follow the four principles in CLAUDE.md (think first, simplicity, surgical changes, goal-driven)

- ⚠️ **Ask first:**
  - Adding new Cargo dependencies
  - Modifying ADRs or architecture docs under `docs/`
  - Modifying CI workflows (`.github/workflows/`)
  - Modifying Dockerfile or deployment scripts
  - Large-scale refactoring of `src/main.rs` (~2700 lines)

- 🚫 **Never do:**
  - Edit anything under `target/`
  - Commit secrets or credentials (registry passwords, TLS private keys)
  - Skip tests before committing
  - Write narrative comments in code
  - Delete pre-existing code that your changes did not make unused
