# AGENTS.md

## Repository Purpose

This repository is a single-crate Rust CLI named `teams` for Microsoft Teams automation through Microsoft Graph. It is designed for AI agents, scripts, and CI systems that need deterministic subprocess behavior rather than an interactive bot framework.

The core contract is:

- Commands should emit machine-readable JSON when stdout is not a TTY.
- Successful JSON output is wrapped in `{ "success": true, "data": ..., "metadata": ... }`.
- Errors are wrapped in `{ "success": false, "error": ..., "metadata": ... }` for JSON output.
- Exit codes are stable and map to error categories in `src/error.rs`.
- Logs and tracing go to stderr so stdout remains parseable.

The crate builds one binary:

```bash
cargo build
cargo run -- --help
cargo run -- team list --output json
```

## High-Level Architecture

- `src/main.rs` parses the root CLI, initializes tracing, loads config, dispatches commands, and exits with `TeamsError::exit_code()`.
- `src/cli/` contains Clap command definitions and command handlers. Each command module usually resolves auth, constructs a `GraphClient`, calls `src/api/`, and prints via `src/output/`.
- `src/api/` contains Microsoft Graph endpoint wrappers. Keep HTTP behavior centralized in `src/api/client.rs`; endpoint URL builders live in `src/api/endpoints.rs`.
- `src/models/` contains Serde models for Microsoft Graph resources and request bodies.
- `src/auth/` contains OAuth flows, token conversion, and OS keyring storage.
- `src/output/` contains JSON envelope, plain text, table, and progress output helpers.
- `src/config.rs` handles TOML config loading/saving and credential/profile resolution.
- `src/listen/` is a Hyper webhook listener for Graph change notifications. It prints notifications as NDJSON.
- `tests/cli.rs` contains black-box CLI tests using `assert_cmd`.
- `docs/teams-cli-prd.md` and `README.md` describe intended product behavior and command surface.
- `docs/README.md` is the documentation index for quickstarts, auth, commands, examples, FAQ, troubleshooting, use cases, and release readiness.
- `docs/auth-implementation-plan.md` is the commercial auth direction: delegated Graph for CLI user actions, BYO app support for locked-down tenants, and Teams bot proactive messaging for unattended posting.
- `docs/man/` contains shipped man pages:
  - `teams.1` for command reference.
  - `teams-config.5` for config file format.
  - `teams-auth.7` for delegated auth, BYO apps, client credentials, and commercial auth direction.
  - `teams-agent-contract.7` for JSON output, exit codes, and live validation guidance.
  - `teams-examples.7` for practical copyable examples.

## Important Runtime Contracts

Do not break the agent-facing output contract. `OutputFormat::detect` in `src/output/mod.rs` defaults to human output for an interactive TTY and JSON for piped/programmatic stdout. Global `--output json|human|plain` overrides detection.

`TeamsError` in `src/error.rs` controls error codes:

- `0`: success
- `1`: general error
- `2`: invalid input
- `3`: auth/token error
- `4`: permission denied
- `5`: not found
- `6`: rate limited
- `7`: network error
- `8`: server error
- `10`: config/keyring error

When adding errors, update `exit_code()` and `error_code()` together and add tests.

## Auth and Configuration

Credential resolution is intentionally predictable:

- Client ID and tenant ID: CLI flag, then `TEAMS_CLI_CLIENT_ID` or `TEAMS_CLI_TENANT_ID`, then config profile.
- Client secret: CLI flag, then `TEAMS_CLI_CLIENT_SECRET`.
- Access token for normal commands: `TEAMS_CLI_ACCESS_TOKEN`, then OS keyring token for the selected profile.

Config lives under the platform config directory in `teams-cli/config.toml`, unless `--config` is provided. Config profiles are resolved by `config::resolve_profile`; `--profile` overrides the configured default profile.

Auth flows:

- Device code for SSH/headless with human approval. This is the recommended delegated flow for normal Teams message posting and now defaults to the OSO public client app when no client ID is supplied.
- Authorization code + PKCE as the default interactive login, also using the OSO public client app unless overridden.
- Client credentials for app-only Microsoft Graph operations that explicitly support application permissions. Do not assume client credentials can send normal Teams chat/channel messages.

Never commit or log real tokens, tenant IDs, client secrets, or private test credentials.

## Graph Client Behavior

All Microsoft Graph calls should go through `GraphClient` in `src/api/client.rs`. It provides:

- Bearer token injection.
- Request timeout from config.
- Retries for network failures, HTTP 429, and HTTP 5xx.
- `Retry-After` handling for rate limits.
- Pagination through `get_paged` and `get_all_pages`.
- Raw byte helpers for file upload/download.

Use endpoint builders from `src/api/endpoints.rs` rather than constructing Graph URLs inline in command handlers. Existing endpoints mostly target `https://graph.microsoft.com/v1.0`; reactions currently use beta endpoints.

## Command Implementation Pattern

Most command modules follow this shape:

1. Define a `#[derive(Subcommand)]` enum for Clap.
2. Implement `pub async fn run(...) -> Result<()>`.
3. Resolve a token with `auth::resolve_token(profile)?`.
4. Create `GraphClient::new(token, &config.network)?`.
5. Validate command-specific inputs and build request models.
6. Call `api::<domain>::...`.
7. Print through `output::print_success`, `print_success_list`, or a table helper.

For paginated list commands, pass `&PaginationOpts` from `src/cli/mod.rs` and use `client.get_paged` in the API layer.

For new command domains, update:

- `src/cli/mod.rs` with the module, enum variant, and dispatch arm.
- `src/api/mod.rs` if adding an API module.
- `src/api/endpoints.rs` for URL builders.
- `src/models/mod.rs` if adding model modules.
- `README.md`, tests, and shell help expectations where relevant.

## Current Command Areas

The CLI currently covers:

- `auth`: login, status, list, switch, logout, token export.
- `user`: me, get, list.
- `config`: init, show, get, set, path, profiles.
- `team`: list/get/create/update/delete/clone/archive/unarchive/member operations.
- `channel`: list/get/create/update/delete/member operations.
- `message`: send/list/get/reply/update/delete/react/unreact/pin/unpin.
- `chat`: list/get/create/update/hide/unhide/member operations.
- `presence`: get, batch get, set, clear, status message.
- `search`: messages, users, teams.
- `tag`: list/get/create/update/delete/member operations.
- `meeting`: list/get/create/update/delete/join URL/attendance.
- `notify`: Teams activity notifications.
- `app` and `tab`: installed apps and channel tabs.
- `file`: channel file list/get/upload/download/delete/share.
- `subscribe`: Graph change notification subscriptions.
- `listen`: local webhook listener that emits NDJSON notifications.
- `completions`: shell completion generation.

## Output Notes

JSON output uses envelope models from `src/models/common.rs`. Human output is not uniformly table-based yet: several commands pretty-print JSON unless they implement custom table rows. Plain output prints object key/value lines or TSV-like lists.

Progress bars are in `src/output/progress.rs`; avoid progress output for JSON stdout. Tracing already writes to stderr.

`auth status` is special: when unauthenticated, it prints an `authenticated: false` success payload and exits with code `1`. Existing tests depend on that behavior.

## Testing and Verification

Useful local checks:

```bash
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo audit
cargo build --release
```

`cargo test --features integration` is reserved for auth-backed integration work and may require Microsoft Graph credentials. Prefer unit tests beside changed code and black-box CLI tests in `tests/cli.rs`. For HTTP behavior, `wiremock` is already a dev dependency and is the preferred approach.

CI runs on GitHub Actions:

- `cargo check --all-targets`
- `cargo fmt -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets` on Ubuntu, macOS, and Windows
- Release builds package Linux, macOS, and Windows binaries with `docs/man/` man pages.

## Development Gotchas

- This is a binary crate, but many unit tests live inside source modules.
- `RUSTFLAGS="-D warnings"` is set in CI, so dead code and unused imports can fail builds.
- Keep stdout clean for JSON-producing commands; diagnostics belong on stderr.
- Preserve deterministic exit codes. Agents may branch on `$?`.
- Avoid direct `reqwest` calls outside auth flows and `GraphClient`.
- Microsoft Graph permissions differ by tenant and auth type. A compile-time pass does not prove a command is usable with every auth flow.
- Webhook subscriptions require an HTTPS public endpoint; `teams listen` only runs the local HTTP listener.
- Most tests do not hit Microsoft Graph. Add mocked tests for API behavior instead of requiring live credentials.
- Keep global option names reserved. In particular, command-specific file paths must not reuse global `--output`, which is the JSON/human/plain output selector.
- Keep `README.md`, `docs/man/teams.1`, and CLI help in sync. If a documented flag form exists in README, add a CLI regression test for it.
- `cargo audit` may report advisory warnings that are not failing vulnerabilities. Treat actual vulnerabilities as release blockers and document any warning accepted for release.

## Pre-Live-Test Gaps

These are the main gaps before claiming customer-ready Microsoft Teams behavior:

- Live Microsoft Graph behavior is not validated by local tests. Use a dedicated test tenant/profile and the checklist in `teams-agent-contract(7)` before using production data.
- Microsoft Graph permissions vary by auth flow. Verify each advertised command with the intended commercial auth mode, especially client credentials versus delegated user auth.
- Destructive commands (`team delete`, `archive`, `unarchive`, `app uninstall`, subscription delete, message/file delete) require explicit test targets and cleanup plans.
- The local environment may not support Windows cross-checking without a Windows Rust target. CI is configured to run tests on `windows-latest`; do not skip that signal for release.
- The webhook listener is plain HTTP and requires an HTTPS reverse proxy for real Graph subscription callbacks.

## Before Finishing Changes

For most changes, run at least:

```bash
cargo fmt -- --check
cargo test --all-targets
```

For command, model, API, or shared error changes, also run:

```bash
cargo clippy --all-targets -- -D warnings
cargo check --all-targets
```

If you change CLI help text, command names, output envelopes, or exit codes, update `README.md`, `docs/man/`, `CLAUDE.md` if still present, and `tests/cli.rs` as needed.
