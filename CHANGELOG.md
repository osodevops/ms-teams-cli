# Changelog

## Unreleased

### Added

- `teams message send --subject TEXT` sets the subject line on a channel root message — the bold title Teams renders above the body, the same field the client offers behind "Add a subject". Channel sends only: chat messages have no subject, so `--subject` with `--chat` (or without `--channel`) is rejected as invalid input before anything is sent.

### Fixed

- `message list` and `message get` no longer drop the `subject` of a message. The `ChatMessage` model had no `subject` field, so a channel root message's subject — returned by Graph on both reads — silently vanished from every output: a message posted with a subject read back without one. Messages without a subject are unchanged and gain no `"subject": null` noise.

## v0.6.0 - 2026-08-30

### Added

- `teams --help-json` emits the whole command tree as JSON: every command's path, summary, usage, and command-specific flags with their value names, defaults, environment variables, and whether they are required. This is what the documentation site consumes to regenerate its command reference on each release. Global options and clap's synthesised `help` subcommands are excluded, since the reference documents globals once and the `help` entries are not part of the command surface.

### Fixed

- The documentation site's command reference can regenerate again. Its workflow has always called `teams --help-json`, which did not exist, so every scheduled run since at least 2026-07-06 failed and `reference/command/**` stayed frozen at v0.3.0. A release now also notifies the docs repository, which previously received no `cli-released` event at all, so the reference and the site's version badge both refresh on release.

## v0.5.0 - 2026-08-30

### Added

- `teams message send --mention USER` (repeatable) tags a person with a real Teams @mention, in chats and channels. Graph only keeps a mention when the body's `<at id="N">` elements are synchronized with a top-level `mentions` array, so the CLI builds both: each value (Entra object ID or UPN) is resolved through Graph, display names are HTML-escaped into the `<at>` prefix in flag order, duplicates collapse to one mention, and a plain-text body is promoted to HTML safely. A mention on its own counts as a body. Raw `<at>` markup in an HTML body without `--mention` is rejected before anything is sent, and `message list`/`get` retain any mentions Graph returns.
- `presence get` output now includes `statusMessage.publishedDateTime`, a documented Graph property that was previously discarded during deserialization.
- `teams auth list` reports, for each profile, the signed-in `user`, `tenant_id`, and `auth_type` (`delegated`, `app-only`, or `unknown`) decoded from the stored token's claims, plus the stored token's `expires_at`, without any network call. A profile whose token cannot be read or decoded is still listed with those fields `null`. This resolves #54.

### Changed

- Refreshed the Rust dependencies: the rust-minor group (clap, clap_complete and others), `base64` 0.22 → 0.23, `toml` 0.8 → 1.1, and `comfy-table` 7.2 → 8.0. comfy-table 8 turns the presets into `TableStyle` constants and replaces `Table::load_preset` with `Table::load_style`; table rendering is unchanged.
- `teams auth list` now emits `profiles` as an array of objects rather than an array of profile-name strings. A consumer reading `.data.profiles[]` as a string needs `.data.profiles[].name` instead. On a terminal the command prints a table, with the active profile marked `*`, in place of the raw JSON it used to show.
- `teams presence set --expiration` is now checked before the request is sent. Microsoft Graph accepts an ISO 8601 duration from `PT5M` to `PT4H`; a malformed or out-of-range value now fails as invalid input (exit 2) with the bounds in the message, instead of costing a round trip and returning a 400 to interpret. The `--availability` and `--activity` help text now names the five pairs `setPresence` actually accepts, rather than `Offline` and `InAMeeting`, which only occur when reading a presence. This resolves #81.

### Fixed

- User lookups now percent-encode the identifier, so a guest UPN containing `#` (`name_domain#EXT#@tenant.onmicrosoft.com`) is no longer truncated at the `#` and read as a URL fragment.
- `teams message send --adaptive-card` now works. Microsoft Graph requires the message body to reference each attachment by id, and the id was generated inside the send path and never written into the body, so every card was rejected with `400 BadRequest: Body does not contain marker for attachment with Id ...` — and because the id never left the function, a caller could not add the marker themselves. The body now carries the marker, and is promoted to HTML the same way the `--attach` path does, escaping a plain-text body rather than concatenating markup onto it. `--adaptive-card` no longer requires `--body`, since the body only has to carry the marker. This resolves #85.
- A `PERMISSION_DENIED` now names the permissions the token carries. Microsoft Graph refuses an under-permissioned request with 403 and, in the case that prompted this, an empty `message`, so the failure said nothing about what was missing and was indistinguishable from a genuine authorization refusal. A 403 now lists the token's delegated scopes, or its application roles when it is app-only, and points at `teams auth doctor` and `teams auth login` — or, for an application role, at the administrator who must grant it, since logging in again cannot add one. An opaque token yields no claims and gets no hint. This resolves #80.
- A response the CLI cannot deserialize now reports what serde objected to. `Failed to parse API response: error decoding response body` named neither the offending value nor where it sat, because `reqwest::Error` keeps serde's account one level down its source chain and `Display` never reaches it; the message now reads `... error decoding response body: invalid type: map, expected a string at line 1 column 66`. This covers every Graph response the CLI deserializes and all five token-exchange paths across the three login flows. This resolves #79.
- `teams presence set`, `presence status` and `presence clear` now work with a default delegated login. All three Graph calls require `Presence.ReadWrite`, which the built-in scope set did not request, so every presence write returned 403. Microsoft does not mark that delegated scope admin-consent required, so it joins the defaults. An existing session keeps the scopes it was granted — run `teams auth login` again to consent to the new one. This resolves #70.
- `teams presence clear` no longer fails with `The SessionId field is required`, and `presence set` no longer opens a presence session under a fresh random UUID that nothing could later clear. Graph keys a presence session to the application that owns it, so both commands now send that application's ID as `sessionId` — a configured `client_id` when there is one, otherwise the `azp`/`appid` claim of the token itself — and report it back. Graph's 404 for "no such session" is reported as `no_presence_session` rather than an error, so a second clear or a retry after a lost response succeeds. This resolves #71.
- The presence write commands now reject an app-only token locally, as the message write commands already did, instead of sending a request that cannot succeed against `/me`.
- `teams presence get` no longer fails with `API error (200): Failed to parse API response` when the target's Teams status message carries an expiry. Microsoft Graph sends `statusMessage.expiryDateTime` as a `dateTimeTimeZone` object, not a string, so both `GET /me/presence` and `GET /users/{id}/presence` failed to deserialize for any account with an expiring status message. This resolves #69.
- On Windows, `teams auth login` no longer fails with `KEYRING_ERROR: ... longer than platform limit of 2560 chars` after a successful sign-in. Credential Manager caps a credential at 2560 bytes and a Microsoft Graph token bundle is routinely larger, so the serialized token is now split across `<profile>:token:<n>` entries with a `<profile>:token` header; `auth logout` removes every piece. macOS and Linux keep a single keychain item as before. This resolves #67.
- `teams auth logout` now reports a failure to delete the stored token instead of silently succeeding and leaving it in the keyring. A profile with no stored token still logs out cleanly.
## v0.4.0 - 2026-08-19

### Added

- `TEAMS_CLI_PROFILE` environment variable for selecting the credential profile, with the same precedence as other auth environment variables: `--profile` flag, then `TEAMS_CLI_PROFILE`, then the config's `default.profile`, then `default`. This resolves #53.
- `teams message react` and `teams message unreact` accept `--chat <chat-id>` for one-on-one and group chat messages, as an alternative to the `--team`/`--channel` pair. This resolves #62.
- `message list` and `message get` now include each message's `reactions` (`reactionType`, `displayName`, `createdDateTime`, `user`).
- `teams message update` accepts `--chat <chat-id>` to edit one's own chat messages, as an alternative to the `--team`/`--channel` pair.

### Changed

- Browser login now sends `prompt=select_account`, so the identity platform always shows the account picker instead of silently reusing an existing browser session. This makes it possible to sign a second account into another profile with `teams --profile <name> auth login`. This resolves #52.
- Reactions call the Microsoft Graph v1.0 `setReaction`/`unsetReaction` actions instead of beta.
- Reaction names are translated to the emoji character Graph requires on writes: the classic `like`, `heart`, `laugh`, `surprised`, `sad`, `angry` (which Graph now rejects by name with HTTP 400) plus `thumbsup`, `thumbsdown`, `eyes`, `tada`, `rocket`, `fire`. Any emoji character passes through unchanged.
- Refreshed the Rust dependency lockfile (rust-minor group) and bumped `h2` to 0.4.16 for RUSTSEC-2026-0258. GitHub Actions pins updated to `actions/checkout` v7.0.1, `Swatinem/rust-cache` v2.9.2, and `softprops/action-gh-release` v3.0.2.

### Fixed

- An explicit `--profile default` now addresses the profile named "default" even after `teams auth switch` has set another config default. Previously the flag's default value was indistinguishable from an explicit one, so the switched profile shadowed the literal "default" profile and it became unreachable from the command line. This resolves #55.
- Token storage no longer deletes and recreates the keyring item on every write. On macOS the recreation discarded the item's access control list, so an "Always Allow" grant was revoked by the next silent token refresh and the keychain prompt returned within the hour. Items are now updated in place, which preserves the grant. This resolves #51.
- A closed stdout pipe (for example `teams completions bash | head`) no longer panics with exit 101. Stdout writes are centralized in `output::write_stdout` / `write_stdout_line`; a broken pipe is treated as normal early termination (exit 0, nothing on stderr), and a command that fails while its pipe closes still exits with its real error code. This resolves #59.
- `teams message update` no longer reports "Failed to parse API response" on a successful edit. For delegated callers Microsoft Graph answers the PATCH with `204 No Content` for chat and channel messages alike; the command now sends a no-content PATCH and reads the message back to show the new text. If the read-back fails the command still succeeds and returns `{"id": ..., "updated": true, "readBackError": ...}`.
- Homebrew publication is now verified end to end: release jobs fail if the tap does not publish all four platform URLs with the release checksums, instead of treating an accepted but unhandled dispatch event as success.

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
