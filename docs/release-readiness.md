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
- Stored token expiry is handled through refresh-token redemption when a refresh token is available. `AUTH_TOKEN_EXPIRED` still means the refresh token is missing, expired, revoked, or rejected by the identity platform.

Entra app registration status as of 2026-05-27:

- OSO public client app is multi-tenant (`AzureADMultipleOrgs`).
- Public client redirect URI is configured as `http://localhost:8400/callback`.
- Public client/device-code fallback is enabled.
- Implicit grant access-token and ID-token issuance are disabled.
- Home tenant admin consent has been granted for the current delegated scope bundle.
- Homepage and marketing URL are set to `http://msteamscli.com/`.
- Privacy URL is set to `https://oso.sh/privacy-policy/`.
- Support URL is set to `https://oso.sh/contact/`.
- App logo is uploaded from the OSO website asset.

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

## Release automation checklist

The full release path is version-bump driven, not PR-merge driven:

1. Open a release PR that updates `Cargo.toml` to the next version.
2. Keep `Cargo.lock` aligned for the root `teams-cli` package version.
3. Add a matching `CHANGELOG.md` entry.
4. After the PR is merged to `main`, confirm `.github/workflows/auto-tag.yml` runs successfully.
5. Confirm `auto-tag.yml` creates the `vX.Y.Z` tag.
6. Confirm the tag starts `.github/workflows/release.yml`.
7. Confirm the release workflow completes all build targets, creates checksums, publishes the GitHub Release, and runs the Homebrew tap and Scoop bucket update jobs.

Important details:

- Merging a feature PR that does not change `Cargo.toml` only runs CI on `main`; it does not create a release.
- `auto-tag.yml` is path-filtered to `Cargo.toml` and only tags when the package version changes.
- The tag push is what triggers `release.yml`.
- The release workflow currently builds:
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `x86_64-unknown-linux-musl`
  - `aarch64-unknown-linux-musl`
  - `x86_64-pc-windows-msvc`

Distribution follow-up as of 2026-06-21:

- `release.yml` sends a `repository_dispatch` event to `osodevops/homebrew-tap`.
- `release.yml` sends a `repository_dispatch` event to `osodevops/scoop-bucket`.
- The tap repository currently has no workflow listening for that dispatch event.
- Until that automation exists, update `osodevops/homebrew-tap` manually after each CLI release.
- Use the published `checksums-sha256.txt` from the GitHub Release to update `Formula/teams-cli.rb`.
- Verify the remote formula points at the new release URLs and checksums.
- The Scoop bucket has an `update-teams-manifest` workflow listener. Verify `bucket/teams.json` points at the new Windows release URL and checksum after each release.

Known CI maintenance item as of 2026-06-04:

- GitHub Actions is warning that Node.js 20 actions are deprecated.
- Update pinned actions used by CI/release before GitHub's June 16, 2026 Node 24 default switch.
- Watch especially `actions/checkout`, `actions/upload-artifact`, `actions/download-artifact`, and `softprops/action-gh-release`.

## GitHub Actions supply-chain checklist

For every GitHub Actions dependency update:

1. Verify the owner and repository are unchanged.
2. Verify the target tag exists in the official action repository.
3. Resolve the tag to the underlying commit.
4. Pin the workflow to the full 40-character commit SHA, not the tag.
5. Keep the trailing version comment accurate, for example `# v7.0.1`.
6. Check `action.yml` for the runtime. Prefer Node 24 compatible action versions.
7. Read the release notes for behavior changes, new inputs, permission changes, or token handling changes.
8. Keep workflow `permissions` at least privilege. Do not give write permissions to build/test jobs.
9. Set `persist-credentials: false` on `actions/checkout` unless a later step explicitly needs checkout's persisted git credentials.
10. Do not merge a Dependabot Actions PR if it changes the action owner/repository, points to a fork, removes SHA pinning, or leaves comments inconsistent with the reviewed version.

Preferred repository setting:

- Require actions and reusable workflows to be pinned to a full-length commit SHA at the repository or organization level.

Dependabot is configured to group GitHub Actions updates into one PR so the complete workflow supply-chain diff can be reviewed together.

## Commercial release blockers

These must be resolved before marketing this as production-ready for external customers:

1. Publisher verification for the OSO Entra app.
2. Windows live validation using Windows Credential Manager.
3. Controlled write/read smoke test in a dedicated Teams test channel.
4. Documented admin-consent onboarding flow for customer tenants.
5. Clear policy for unsupported Graph operations, tenant restrictions, and destructive commands.
6. Security review of token storage, logs, and exported token behavior.
7. Versioned release notes and upgrade guidance.
8. Public website HTTPS fixed for `https://msteamscli.com/`; HTTP is live, but the current TLS certificate does not cover the hostname.
9. Terms of service URL published and added to the Entra app branding.

## Microsoft official trust checklist

For the current CLI-only Graph app, Microsoft Teams Store submission is not required. The release should still complete:

- Entra publisher domain set to OSO's verified domain.
- Microsoft AI Cloud Partner Program account linked for publisher verification.
- Publisher verification completed so consent prompts show a verified publisher.
- App display name, logo, homepage (`http://msteamscli.com/`), privacy policy, and support URLs set in Entra branding.
- Terms URL still needs to be published and added.
- Admin consent URL documented for customer tenants.
- Permission list documented with a short purpose for each delegated scope.

Microsoft 365/Teams app publisher attestation or certification becomes relevant if OSO later ships a Teams app or bot through Teams Store/AppSource.

## Kafka Backup parity review

Reviewed against `osodevops/kafka-backup` on 2026-05-26.

Already covered here:

- GitHub Actions CI on Linux, macOS, and Windows.
- GitHub release assets with SHA256 checksums.
- Homebrew formula in `osodevops/homebrew-tap`.
- Scoop manifest in `osodevops/scoop-bucket`.
- README, docs, man pages, changelog, license, contribution guide, and security policy.
- Agent-focused repo guidance in `AGENTS.md`.

Added from that comparison:

- GitHub issue templates for bug reports and feature requests.
- Dependabot configuration for Cargo and GitHub Actions.
- Public homepage metadata in Cargo and GitHub repository settings.

Still intentionally not added:

- cargo-dist shell and PowerShell installers.
- Docker image publishing.
- Demo repository with runnable customer scenarios.

Those are useful distribution improvements, but they should be implemented deliberately rather than mixed into the current proven release workflow.

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
