# Changelog

## Unreleased

### Added

- Automatic refresh-token redemption. When the stored access token is expired (or within a short skew window of expiring), the CLI now silently exchanges the persisted `refresh_token` for a fresh access token via the OAuth2 `refresh_token` grant and updates the keyring, instead of failing with `AUTH_TOKEN_EXPIRED` roughly an hour after login. The previous re-login behaviour remains as a fallback when no refresh token is stored or the refresh request is rejected. This resolves the standing "automatic refresh-token handling" known limitation (#16).

## v0.2.4 - 2026-06-04

### Changed

- Reduced the default delegated Graph login scopes by removing `ChannelMessage.Read.All`.
- Updated `auth consent-url` and `auth doctor` to emit Microsoft identity platform v2 admin consent URLs with explicit scopes and redirect URI diagnostics.
- Documented the channel-message read consent path separately from the default chat/message send workflow.

## v0.2.3 - 2026-05-26

### Fixed

- Corrected the Homebrew install command in the README to use the published `teams-cli` formula.

## v0.2.2 - 2026-05-26

### Fixed

- Made the custom config path CLI test portable on Windows by validating parsed JSON output instead of matching escaped path text.

## v0.2.1 - 2026-05-26

### Fixed

- Fixed pinned GitHub Actions Rust toolchain setup by passing `toolchain: stable` explicitly.

## v0.2.0 - 2026-05-26

### Added

- Built-in OSO delegated public client app as the default for browser and device-code login.
- `teams auth consent-url` for customer admin consent onboarding.
- `teams auth doctor` for profile, app, consent URL, and token diagnostics.
- Comprehensive documentation under `docs/`, including quickstarts, auth guide, command reference, examples, FAQ, troubleshooting, use cases, and release readiness.
- New man pages: `teams-auth(7)` and `teams-examples(7)`.

### Changed

- Normal Teams message mutation commands now require delegated tokens and reject app-only tokens before calling Graph.
- Release archives now include man pages and the Markdown documentation set.
- CLI tests avoid touching the real OS keyring.
- `team list` avoids unsupported OData customization on `/me/joinedTeams`.

### Known Limitations

- Automatic refresh-token handling still needs to be completed before a broad commercial release.
- The OSO Entra app must be publisher verified before external enterprise rollout.
- Teams Store submission is not required for this CLI-only Graph app, but will be relevant for a future Teams app/bot package.
