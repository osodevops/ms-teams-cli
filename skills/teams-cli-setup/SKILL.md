---
name: teams-cli-setup
description: "Install and sign in to the teams CLI (Microsoft Teams via Microsoft Graph) and learn its agent output contract. Use when asked to install teams-cli, log in to Teams, fix 'not authenticated' or AUTH_TOKEN_EXPIRED, run auth doctor, switch profiles, configure a bring-your-own Entra app, set up CI or unattended access, or interpret JSON envelopes, exit codes, pagination and stderr logs from teams commands."
---

# Teams CLI Setup

Install `teams`, sign in once, and understand how every command reports results. Other teams-cli skills assume this is done.

## Install

```bash
# Homebrew (macOS/Linux)
brew install osodevops/tap/teams-cli

# Scoop (Windows)
scoop bucket add osodevops https://github.com/osodevops/scoop-bucket
scoop install teams

# From source
cargo install --git https://github.com/osodevops/ms-teams-cli
```

Pre-built binaries are on https://github.com/osodevops/ms-teams-cli/releases. Check with `teams --version`.

## First sign-in

Messages are sent as the signed-in user, so use a delegated login. No client ID is needed: the default is OSO's public client app.

```bash
teams auth login
teams auth login --device-code
teams auth status --output json
teams auth doctor --output json
```

- `auth login` opens a browser (authorization code + PKCE). Use it on a desktop.
- `--device-code` prints a code for a human to approve on another device. Use it over SSH or headless.
- The human must complete the approval. Do not try to automate it.
- If `auth doctor` reports missing admin consent, `teams auth consent-url` prints the URL for a tenant admin.

Bring your own Entra app when a tenant blocks the default app:

```bash
teams auth login --device-code --client-id <client-id> --tenant-id <tenant-id>
teams auth login --client-id <client-id> --tenant-id <tenant-id> --redirect-uri http://localhost
```

The browser login needs a loopback redirect URI registered on the app (the default is `http://localhost/callback`). Pass the registered one with `--redirect-uri`; without a port, a free one is picked at each login. A successful delegated login saves the client ID, tenant ID and redirect URI in the profile, so later logins reuse them. Environment variables are not saved.

## Sessions and profiles

```bash
teams auth refresh
teams auth list
teams auth switch <profile>
teams auth logout
teams --profile <profile> user me --output json
teams config path
teams config show --output json
teams config set <key> <value>
```

`--profile` (or `TEAMS_CLI_PROFILE`) picks a profile per command. `--config <path>` selects another config file. `teams config init` writes a default file. Run `teams config --help` for the keys. `auth logout` removes the stored sign-in; run it only when asked.

## CI and unattended use

- `TEAMS_CLI_ACCESS_TOKEN` supplies a token for normal commands. Take it from a secret store. Never print it, echo it or commit it.
- `TEAMS_CLI_TOKEN_STORE=file` keeps tokens in `0600` files under the config directory instead of the OS keyring. Use it for daemons and SSH.
- `TEAMS_CLI_DISABLE_KEYRING=1` is for tests only. It stops tokens being stored.
- `teams auth login --client-credentials` gives an app-only token (`TEAMS_CLI_CLIENT_ID`, `TEAMS_CLI_TENANT_ID`, `TEAMS_CLI_CLIENT_SECRET`). It can NOT send chat or channel messages as a user, and many Teams reads fail with it. Delegated login is the path for messaging.
- Pass the secret by environment variable, not `--client-secret`, which leaks into process listings.

## Output contract

- Stdout is JSON when it is not a TTY. `--output json|human|plain` overrides. Always pass `--output json` when parsing.
- Success: `{"success": true, "data": ..., "metadata": {...}}`.
- Failure: `{"success": false, "error": {"code": "...", "message": "..."}, "metadata": {...}}`.
- Logs and progress go to stderr, so stdout stays parseable. `RUST_LOG=teams=debug` adds detail there.
- A closed pipe (`| head -1`) is a normal exit 0.

| Exit | Meaning | Do this |
|------|---------|---------|
| 0 | Success | Parse `.data` |
| 1 | General error | Read `.error.message` |
| 2 | Invalid input | Fix the command |
| 3 | Auth or token error | Sign in again |
| 4 | Permission denied (403) | Check scopes and consent, or stop |
| 5 | Not found (404) | Re-check the id |
| 6 | Rate limited (429) | Back off, then retry |
| 7 | Network error | Retry reads with backoff; a write may have gone through, so check first |
| 8 | Server error (5xx) | Retry reads with backoff; for writes, check first |
| 10 | Config or keyring error | Check config, env vars, keyring |

`teams auth status` exits 1 when unauthenticated and still prints `{"authenticated": false}` as a success payload. A locked keychain is exit 10, not 3.

## Pagination

List commands return one page. Read `metadata.has_more`; when true, `metadata.next_link` is Graph's next-page link. Add `--all-pages` to fetch everything (`has_more` is then false). `--page-size <n>` sets the page size.

## Troubleshooting

- 403 or exit 4: the error lists the scopes the token holds. Run `teams auth doctor`, then `teams auth login` to consent to the missing scope, or `teams auth consent-url` when only an admin can grant it. After admin consent, `teams auth refresh` picks it up without a browser.
- macOS keychain prompts on every run: each unsigned build is a new app. Use a release build or `TEAMS_CLI_TOKEN_STORE=file`.
- Windows or Linux keyring errors (exit 10): unlock or install a keyring service, or use `TEAMS_CLI_TOKEN_STORE=file`.
- Exit 6: honor the wait, avoid parallel fan-out into large tenants.
- The browser page shows `AADSTS50011`: the redirect URI is not registered on the app, and the CLI keeps waiting. Stop it, then pass the registered URI with `--redirect-uri`, or use `--device-code`.
- Browser login fails with `Sign-in failed: <error>`: the user declined, or the tenant blocked the app. Read the description; `auth consent-url` helps when admin consent is missing. With no callback at all, the login gives up after 10 minutes.

Full details: https://github.com/osodevops/ms-teams-cli/tree/main/docs (`troubleshooting.md`), and `man teams-agent-contract`.

## Related skills

- `teams-cli-messaging`: read, send and reply in chats and channels.
- `teams-cli-admin`: teams, channels, tags, apps and tabs.
- `teams-cli-automation`: presence, meetings, notifications, webhooks.
