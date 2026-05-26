# Release readiness

This checklist separates local correctness, internal release candidate quality, and public commercial readiness.

## Current status

Local verification passed on macOS:

```bash
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
mandoc -Tlint docs/man/teams.1 docs/man/teams-config.5 docs/man/teams-agent-contract.7
git diff --check
```

Live read-only validation passed against the OSO profile:

- Device-code login.
- `auth doctor`.
- `auth status`.
- `user me`.
- `team list`.
- `chat list`.
- `message list --chat` for accessible chats.
- `presence get`.

Known live behavior:

- Some meeting chats can appear in `chat list` but reject message reads with `403` if the user is no longer in the roster.
- Stored token expiry currently requires manual re-login.

## Internal release candidate gate

Required:

- Clean, reviewed diff.
- All local Rust checks pass.
- CLI help matches docs and man pages.
- Man pages lint clean.
- Generated screenshots, Playwright snapshots, and local temp files are not committed.
- No real tokens, chat contents, tenant secrets, phone numbers, or customer data in docs/tests.
- GitHub Actions passes on Linux, macOS, and Windows.
- Release workflow builds artifacts for supported platforms.

## Commercial release blockers

These must be resolved before marketing this as production-ready for external customers:

1. Publisher verification for the OSO Entra app.
2. Automatic refresh-token handling and tests.
3. Windows live validation using Windows Credential Manager.
4. Controlled write/read smoke test in a dedicated Teams test channel.
5. Documented admin-consent onboarding flow for customer tenants.
6. Clear policy for unsupported Graph operations, tenant restrictions, and destructive commands.
7. Security review of token storage, logs, and exported token behavior.
8. Versioned release notes and upgrade guidance.

## Microsoft official trust checklist

For the current CLI-only Graph app, Microsoft Teams Store submission is not required. The release should still complete:

- Entra publisher domain set to OSO's verified domain.
- Microsoft AI Cloud Partner Program account linked for publisher verification.
- Publisher verification completed so consent prompts show a verified publisher.
- App display name, logo, homepage, privacy policy, and terms URLs set in Entra branding.
- Admin consent URL documented for customer tenants.
- Permission list documented with a short purpose for each delegated scope.

Microsoft 365/Teams app publisher attestation or certification becomes relevant if OSO later ships a Teams app or bot through Teams Store/AppSource.

## Controlled live smoke test

Use a dedicated team/channel with no customer data.

Read-only:

```bash
teams auth doctor --output json
teams user me --output json
teams team list --output json
teams channel list "$TEAM_ID" --output json
teams message list --team "$TEAM_ID" --channel "$CHANNEL_ID" --output json
```

Write:

```bash
SENT=$(teams message send \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --body "teams-cli release smoke test" \
  --output json)

MESSAGE_ID=$(echo "$SENT" | jq -r '.data.id')

teams message reply \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --message-id "$MESSAGE_ID" \
  --body "reply smoke test" \
  --output json
```

File:

```bash
printf 'teams-cli file smoke\n' | teams file upload \
  --team "$TEAM_ID" \
  --channel "$CHANNEL_ID" \
  --stdin \
  --name teams-cli-smoke.txt \
  --output json
```

Cleanup only where tenant policy allows it.

## Windows validation

Run:

```powershell
cargo fmt -- --check
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
teams auth login --device-code
teams auth doctor --output json
teams chat list --output json
```

Validate:

- Config path resolves under `%APPDATA%\teams-cli\config.toml`.
- Token stores in Windows Credential Manager.
- `auth status` returns quickly after login.
- `auth logout` removes the profile.
- PowerShell examples in docs work as written.

## Security review checklist

- `auth token` is documented as sensitive.
- Examples do not encourage writing tokens to shell history.
- Logs are stderr and do not include access tokens.
- JSON examples are sanitized.
- `TEAMS_CLI_DISABLE_KEYRING` is documented as test-only.
- BYO client secrets are environment variables or secret manager values, not config examples.

## Release notes template

```markdown
## teams-cli vX.Y.Z

### Added
- ...

### Changed
- ...

### Fixed
- ...

### Auth and permissions
- ...

### Known limitations
- ...

### Verification
- Linux/macOS/Windows CI: passed
- Live OSO smoke test: passed on YYYY-MM-DD
```
