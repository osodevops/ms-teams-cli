# teams-cli

Rust CLI for Microsoft Teams — agent-first design via Microsoft Graph API.

## Build Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo check                          # Type check only
cargo test --all-targets             # All tests (unit + integration)
cargo test --lib --bins              # Unit tests only
cargo test --features integration    # Integration tests (needs auth)
cargo fmt -- --check                 # Check formatting
cargo clippy --all-targets -- -D warnings  # Lint
```

## Architecture

Single-crate Rust binary. Key modules:

- `src/main.rs` — Entry point, tracing init, config load, CLI dispatch
- `src/cli/` — Clap command definitions and handlers
  - `auth.rs` — login (PKCE, device code, client credentials), status, list, switch, logout, token
  - `team.rs` — list, get, create, update, delete, clone, archive, unarchive, members
  - `channel.rs` — list, get, create, update, delete, members
  - `message.rs` — send, list, get, reply, delete, react, unreact, pin, unpin
  - `chat.rs` — list, get, create, hide, unhide, members
  - `presence.rs` — get, set, clear, status, get-batch
  - `search.rs` — messages, users, teams
  - `tag.rs` — list, get, create, delete, add-member, remove-member
  - `meeting.rs` — list, get, create, delete, join-url, attendance
  - `notification.rs` — send, send-to-team, send-to-chat
  - `app.rs` — list, install, uninstall
  - `tab.rs` — list, create, delete
  - `file.rs` — list, get, upload, download, delete, share
  - `subscribe.rs` — create, list, renew, delete
  - `listen.rs` — webhook listener entry point
  - `config_cmd.rs` — init, show, get, set, path
  - `user.rs` — me, get, list
- `src/api/` — Microsoft Graph HTTP client and endpoint wrappers
  - `client.rs` — GraphClient with retry/backoff, rate-limit handling, pagination
  - `endpoints.rs` — URL builders for all Graph API endpoints
  - `teams.rs`, `channels.rs`, `messages.rs`, `chats.rs`, `presence.rs`, `search.rs`, `tags.rs`, `meetings.rs`, `notifications.rs`, `apps.rs`, `files.rs`, `subscriptions.rs`, `users.rs`
- `src/models/` — Serde data models for Graph API resources
  - `team.rs`, `channel.rs`, `message.rs`, `chat.rs`, `presence.rs`, `search.rs`, `tag.rs`, `meeting.rs`, `notification.rs`, `app.rs`, `file.rs`, `subscription.rs`, `member.rs`, `user.rs`, `common.rs`
- `src/listen/` — Webhook listener HTTP server
  - `mod.rs` — hyper server with Ctrl+C graceful shutdown
  - `handler.rs` — validation token echo, NDJSON notification output, health check
- `src/auth/` — Authentication flows and token management
  - `auth_code_pkce.rs` — Authorization Code + PKCE (browser flow)
  - `device_code.rs` — Device Code flow
  - `client_credentials.rs` — Client Credentials flow
  - `token.rs` — TokenInfo struct, expiry checking
  - `keyring.rs` — OS keyring storage (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- `src/output/` — Output formatters (JSON envelope, human tables, plain text)
- `src/config.rs` — TOML config file management, profile/credential resolution
- `src/error.rs` — TeamsError enum with exit codes per PRD

## Key Design Patterns

### Output Contract
All commands emit a JSON envelope: `{ "success": bool, "data": ..., "metadata": { "request_id", "timestamp", "duration_ms" } }`.
When stdout is a TTY, defaults to human-readable table format. When piped, defaults to JSON.

### Exit Codes
0=success, 1=general, 2=invalid input, 3=auth, 4=permission denied, 5=not found, 6=rate limited, 7=network, 8=server error, 10=config error

### Credential Resolution
CLI flags > env vars (TEAMS_CLI_CLIENT_ID, TEAMS_CLI_CLIENT_SECRET, TEAMS_CLI_TENANT_ID) > config file profiles

### Token Management
- Login stores tokens in OS keyring via `keyring` crate
- Subsequent commands load tokens from keyring — no re-login needed
- Multiple named profiles supported (--profile flag)
- Profile index tracked in keyring for `auth list`

### Graph API Client
- Automatic retry with exponential backoff on 429/5xx
- Respects `Retry-After` header for rate limiting
- Pagination via `@odata.nextLink` with `--all-pages` flag
- `$top` parameter for page size control

### Webhook Listener
- `teams listen --port 8080` starts a hyper HTTP server
- Handles Graph subscription validation (echoes `?validationToken`)
- Outputs change notifications as NDJSON to stdout
- Multi-connection, graceful Ctrl+C shutdown
- Requires HTTPS via reverse proxy (ngrok) for production use

## Environment Variables
- `TEAMS_CLI_CLIENT_ID` — Azure AD application (client) ID
- `TEAMS_CLI_CLIENT_SECRET` — Azure AD client secret
- `TEAMS_CLI_TENANT_ID` — Azure AD tenant ID
- `TEAMS_CLI_ACCESS_TOKEN` — Pre-obtained access token
- `RUST_LOG` — Tracing filter level

## Config
- Config file: `~/.config/teams-cli/config.toml` (Linux) or `~/Library/Application Support/teams-cli/config.toml` (macOS)
- Profiles define client_id, tenant_id, auth_flow per account
- Network section: timeout, max_retries, retry_backoff_base
- Output section: format, color, page_size
