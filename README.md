# Rust Serverless Bot Template

Template repository for scheduled Rust bots running on GitHub Actions.

`Cargo.lock` is committed because this is an application. CI and the bot runner use
`--locked`; update the committed lockfile when dependencies change. CI also runs
the harmless placeholder without Telegram credentials.

Dependency updates form one weekly maintenance batch with a seven-day cooldown.
Native auto-merge waits for `ci-gate` and `workflow-security-gate` on the current
commit, with no review approval or merge cron. Derived repositories must require
both gates, keep the branch up to date, and enable native auto-merge.

## Included
- CI workflow with dependency checks (`.github/workflows/ci.yml`)
- GitHub Actions security workflow with actionlint and Zizmor (`.github/workflows/workflow-security.yml`)
- Release workflow that builds the binary once (`.github/workflows/release.yml`)
- Verified release-binary setup action (`.github/actions/setup-release-binary`)
- Scheduled bot runner (`.github/workflows/run-bot.yml`)
- Keepalive workflow to maintain repository activity (`.github/workflows/keepalive.yml`)

## Release and runtime model

This template follows a build-once, run-many pattern:

1. `release.yml` builds `cargo build --release --locked` when source, dependency
   or toolchain files change on `main`. It publishes the executable and a
   `delivery-manifest.json` to the `latest` release. The manifest records the
   protocol version (`2`), the source commit SHA and the binary's SHA-256.
2. `run-bot.yml` never compiles. It uses `setup-release-binary` to download the
   published binary and manifest, waits for a protocol 2 manifest while a new
   release is being built, verifies the SHA-256 with system tools, and then
   executes the binary directly.
3. A missing or stale manifest fails closed: the runner does not execute an
   older release during the cutover window.

When adapting the template, rename `BINARY_NAME` in both `release.yml` and
`setup-release-binary/action.yml` to match your crate name. Keep the manifest
version unchanged unless you deliberately change the protocol.

## Usage
1. Create a repository from this template.
2. Add required secrets used by your bot (for example `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`).
3. Implement bot logic in `src/main.rs`.
4. Adjust cron in `run-bot.yml`.

## Local Commands
- `cargo fmt --all -- --check`
- `cargo check --tests --benches`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo nextest run --no-tests=pass`
- `cargo audit`
- `cargo deny check advisories bans sources`
- `cargo machete`

Before publishing a derived project, choose and declare its license. License
enforcement is intentionally not enabled by the template's `cargo-deny` policy.
