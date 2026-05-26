# Troubleshooting

## Quick diagnostics

Run these first:

```bash
teams auth doctor --output json
teams auth status --output json
teams config show --output json
```

Add verbose stderr logs:

```bash
RUST_LOG=teams=debug teams chat list --output json
```

## `AUTH_TOKEN_EXPIRED`

Meaning: the keyring token is expired.

Current workaround:

```bash
teams auth login --device-code
```

Release-readiness note: automatic refresh-token handling still needs to be completed and validated.

## `AUTH_FAILED` or login fails

Check:

- Tenant allows user consent or admin consent has been granted.
- The OSO app is allowed by enterprise app policy.
- BYO profile has `auth_app = "byo"` and a valid `client_id`.
- Browser PKCE redirect `http://localhost:8400/callback` is allowed.
- Device-code login is used for SSH or remote terminals.

Useful command:

```bash
teams auth consent-url --tenant-id <tenant-id-or-domain> --output json
```

## `PERMISSION_DENIED`

Common causes:

- Admin consent has not been granted.
- The signed-in user is not a member of the team, channel, chat, or meeting roster.
- The Graph API does not support the operation with the token type used.
- A Teams policy blocks the action.
- Client credentials were used for a delegated-only operation.

Check token type:

```bash
teams auth doctor --output json | jq '.data.token.auth_type'
```

For normal message sends, this must be `delegated`.

## App-only token rejected for message commands

Message mutation commands require delegated auth for normal live Teams usage:

```bash
teams auth login --device-code
```

Do not use:

```bash
teams auth login --client-credentials
```

for normal chat/channel message posting.

## `chat list` works but `message list --chat` gets `403`

This can happen with meeting chats. The chat can appear in the list, but message reads fail if the signed-in user is not in the roster for that thread.

Agent behavior:

- Log the chat ID and Graph request ID.
- Skip that chat.
- Continue processing other chats.

Do not treat one inaccessible chat as a global auth failure.

## `team list` returns an empty array

`team list` uses `/me/joinedTeams`. It returns teams where the user is a direct member. It does not mean the user has no chats, meetings, or shared-channel context.

Try:

```bash
teams chat list --output json
teams search teams --query "<name>" --output json
```

## `team list` and `$top`

Microsoft Graph currently says `/me/joinedTeams` does not support OData query parameters to customize the response. The CLI avoids `$top` for this endpoint.

## Windows keyring issues

Symptoms:

- Auth commands hang.
- Credential Manager prompts appear.
- CI tests block when touching real credentials.

Actions:

- Use a normal user session, not a locked-down service desktop, for interactive login.
- Run PowerShell as the same user that will run `teams`.
- Check Windows Credential Manager for `teams-cli` entries.
- In tests only, set `TEAMS_CLI_DISABLE_KEYRING=1`.

PowerShell:

```powershell
$env:TEAMS_CLI_DISABLE_KEYRING = "1"
cargo test --all-targets --all-features
```

Do not set `TEAMS_CLI_DISABLE_KEYRING=1` for real usage; it prevents token storage.

## Linux keyring issues

The `keyring` crate usually needs a Secret Service-compatible backend.

In headless Linux CI, prefer one of:

- Use `TEAMS_CLI_ACCESS_TOKEN` for short-lived tests.
- Run CLI tests with `TEAMS_CLI_DISABLE_KEYRING=1`.
- Install and configure a keyring backend if testing real login.

## Browser login callback fails

Browser login uses PKCE and the redirect URI:

```text
http://localhost:8400/callback
```

Check:

- Port `8400` is free.
- Local firewall allows loopback.
- BYO app registration includes that redirect URI.
- Use `--device-code` if a browser callback is not possible.

## Rate limiting

Exit code `6` means rate limited after retries.

The shared Graph client honors `Retry-After` when Microsoft Graph sends it. For agents, add higher-level backoff and avoid parallel fan-out into large tenants.

## Invalid config

Show the current file path:

```bash
teams config path
```

Inspect config:

```bash
teams config show --output json
```

Recreate a minimal config:

```bash
mv "$(teams config path --output plain | awk '/path:/ {print $2}')" config.toml.bak
teams config init
```

## Raw file bytes in terminal

`teams file download` writes raw bytes to stdout when `--path` is omitted. Use:

```bash
teams file download --team "$TEAM_ID" --channel "$CHANNEL_ID" --file-id "$FILE_ID" --path ./download.bin
```

## Webhook subscription validation

Microsoft Graph requires a public HTTPS notification URL. `teams listen` is local HTTP.

For local testing:

```bash
teams listen --port 8080
```

Expose it with a trusted HTTPS tunnel, then create the subscription with the tunnel URL.
