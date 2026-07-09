# Changelog

## Unreleased

## v0.3.0 - 2026-07-09

### Added

- Per-profile delegated scope configuration for auth flows, including `TEAMS_CLI_SCOPES`, `--scopes` overrides, and scope-aware admin consent URLs.
- `teams auth refresh` for explicitly redeeming a stored refresh token and upgrading delegated scopes without forcing a full login when consent already exists.
- `teams user resolve`, which resolves IDs, UPNs, email addresses, and names using exact `/users` lookup, People API candidates, and an optional chat-roster sweep.
- `teams message attachments list` and `teams message attachments download`: read the images users paste into messages (Graph hosted contents), file attachments stored in SharePoint/OneDrive, and code snippets. `list` returns an indexed inventory; `download` saves items to disk (or stdout with `--path -`) and reports each file's path, size, and MIME type. Works for channel messages, channel thread replies (`--reply`), and chats (`--chat`). Inline images need no scopes beyond message reads; file attachments require the `Files.Read.All` delegated scope and fail with an actionable hint without it.
- `teams message get --with-attachments` embeds the same attachment inventory in the message output under `attachment_items`.
- `teams message send`/`reply` gained `--image` (send a picture inline, like pasting a screenshot — no scopes beyond message sends) and `--attach` (upload a file to OneDrive/SharePoint and attach it — needs `Files.ReadWrite` for chats or `Files.ReadWrite.All` for channels). Both repeat for multiple files; `--body` is optional when media is present. Scope failures explain which storage the upload targets, which scope it needs, and how to grant it; docs/attachments-spec.md carries the full hosted-contents-vs-files explainer.

### Fixed

- CLI tests now isolate config-directory lookup from the developer's real machine config and scrub inherited auth environment variables.
- `teams user resolve` now verifies People API hits through `/users/{userPrincipalName}` before returning a directory object ID, rather than treating the People resource ID as a Microsoft Entra user ID.
- Attachment and inline-image parsing now accepts only Microsoft Graph hosted-content URLs for bearer-token downloads and rejects lookalike external URLs.
- `teams message send --attach` now uses Microsoft Graph's 250MB simple-upload limit for DriveItem uploads instead of incorrectly applying the 4MB JSON request limit.
- `teams message get` no longer silently drops the `contentUrl`, `thumbnailUrl`, and `teamsAppId` fields of message attachments, which made file attachments unresolvable from CLI output.
- `teams completions` now generates completions on a larger worker-thread stack so the expanded command tree does not overflow the default Windows stack.

### Changed

- Refreshed the Rust dependency lockfile, including the `anyhow` advisory fix pulled in by the rust-minor dependency update.

## v0.2.8 - 2026-07-03

### Added

- `teams chat create --members` now accepts an optional per-member role suffix (`<user-id>:guest`) so chats can include Azure AD guest users. Members without a suffix default to `owner`, which is what Microsoft Graph expects for regular tenant users in personal chats. This resolves part of #30.

### Fixed

- Fixed `teams chat create`, which always failed against Microsoft Graph: it POSTed to the list-only `/me/chats` endpoint (HTTP 405) and sent members without the required role (HTTP 400). Chat creation now POSTs to `/chats` with an explicit role per member. This resolves #30.

## v0.2.7 - 2026-06-29

### Fixed

- Fixed `teams chat members list` so it no longer sends the unsupported `$top` query parameter to Microsoft Graph's list-chat-members endpoint. The command still follows `@odata.nextLink` when `--all-pages` is used. This resolves #27.

## v0.2.6 - 2026-06-26

### Fixed

- Fixed `teams channel list` pagination so it no longer sends the unsupported `$top` query parameter to Microsoft Graph's list-channels endpoint. The command still follows `@odata.nextLink` when `--all-pages` is used. This resolves #23.
- Updated the transitive `quinn-proto` lockfile pin to `0.11.15` to resolve `RUSTSEC-2026-0185`.

## v0.2.5 - 2026-06-21

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
