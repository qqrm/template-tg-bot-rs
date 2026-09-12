# Repository Agent Instructions

## Core Principles
- Keep automation simple and deterministic.
- Favor standard crates and clear error handling.
- Treat schedules as production jobs and monitor failures.

## Validation Loop
- `cargo fmt --all -- --check`
- `cargo check --tests --benches`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo nextest run --no-tests=pass`

## Workflow Rules
- Never bypass failing checks on `main`.
- Keep secrets only in GitHub Actions secrets.
