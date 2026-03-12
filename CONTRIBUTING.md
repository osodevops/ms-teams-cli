# Contributing to teams-cli

Thanks for your interest in contributing! This project aims to provide a secure, scriptable Microsoft Teams CLI for developers and AI agents.

- Code: Rust 2021, `clap` v4, async via `tokio`, HTTP via `reqwest`.
- Style: run `cargo fmt` and `cargo clippy -- -D warnings` before pushing.
- Tests: add unit tests near changed code; for HTTP, prefer `wiremock` for integration tests.
- Commits: conventional, clear messages. Small, focused PRs are easier to review.
- Security: never include secrets in tests, examples, or logs.

## Dev setup

```bash
rustup toolchain install stable
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

## Pull Requests

- Write a descriptive title and summary.
- Link related issues.
- Include usage notes and sample JSON if useful.
- Update docs/README where applicable.

## Code of Conduct

This project follows a standard Code of Conduct. Be respectful and inclusive.
