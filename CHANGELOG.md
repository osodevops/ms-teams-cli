# Changelog

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
