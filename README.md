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

1. `release.yml` builds `cargo build --release --locked` for every push to
   `main` (and manual dispatch on main). It publishes a prerelease tagged
   `delivery-<source_sha>` with the executable and protocol 2
   `delivery-manifest.json`. Existing published commit releases are verified,
   not overwritten; builds for different commits do not cancel each other.
2. `run-bot.yml` never compiles. It selects the release by the exact checked-out
   SHA, then verifies protocol, source SHA and binary SHA-256 before installing.
   It waits for publication and fails closed if that exact release is missing.
3. The publisher verifies the downloaded assets without executing the bot.
   A newer release cannot displace the binary for an older in-flight checkout.

The old mutable `latest` release is not used by new runners. Commit releases
are retained for historical runs; remove obsolete generated releases only after
their possible delivery/re-run window has elapsed. An incomplete draft requires
inspection before retrying. No automatic deletion is enabled.

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
