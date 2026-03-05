# Rust Serverless Bot Template

Template repository for scheduled Rust bots running on GitHub Actions.

## Included
- CI workflow (`.github/workflows/ci.yml`)
- Scheduled bot runner (`.github/workflows/run-bot.yml`)
- Keepalive workflow to maintain repository activity (`.github/workflows/keepalive.yml`)
- Codex cleanup workflow (`.github/workflows/codex-cleanup.yml`)

## Usage
1. Create a repository from this template.
2. Add required secrets used by your bot (for example `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`).
3. Implement bot logic in `src/main.rs`.
4. Adjust cron in `run-bot.yml`.

## Local Commands
- `cargo fmt --all -- --check`
- `cargo check --tests --benches`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
