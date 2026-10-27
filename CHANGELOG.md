# Changelog

## Unreleased

## v0.11.0 - 2026-10-27

### Added

- `--redirect-uri`, `TEAMS_CLI_REDIRECT_URI` and the profile's `redirect_uri` select the browser login's redirect URI for `auth login`, `auth consent-url` and `auth doctor`. The login listens on the URI's host and port (both loopback addresses for `localhost`) for a request to the URI's path carrying the login's `state`; the flag is saved to the profile with the IDs. A redirect URI without a port gets a free port from the OS at each login, as MSAL does; the identity platform allows this because it ignores the port of a localhost redirect URI, so `--redirect-uri http://localhost` works for an app registered that way.
- `teams subscribe create --lifecycle-url URL` sets the subscription's `lifecycleNotificationUrl`, which Graph requires for Teams subscriptions longer than one hour. Graph validates that URL too, so `teams listen` answers validation and notifications on `/lifecycle` as well as `/` and `/webhook`, and one tunnel serves both.
- `teams listen --client-state SECRET` (or `TEAMS_CLI_CLIENT_STATE`) drops notifications whose `clientState` is missing or different, with a warning on stderr that does not include either value. Without it, the listener warns at startup that notifications are not authenticated.
- `teams listen --bind ADDR` chooses the listening address, and `--port 0` picks a free port; the startup line names the address actually bound.
- Each `teams listen` line now carries every field Graph sent (`subscriptionExpirationDateTime`, `lifecycleEvent`, `encryptedContent` and so on) and `kind`: `lifecycle` when `lifecycleEvent` is present, else `change`. The README's "Running behind a relay" says what a relay should POST and which statuses to retry.
- Agent skills for AI agents under `skills/`: `teams-cli-setup`, `teams-cli-messaging`, `teams-cli-admin` and `teams-cli-automation`. Install them with `npx skills add osodevops/ms-teams-cli`.

### Changed

- **`teams listen` binds `127.0.0.1` by default instead of `0.0.0.0`.** Pass `--bind 0.0.0.0` to accept connections from other hosts.
- `teams listen` answers notifications with 202 once they are queued for stdout, instead of 200 after printing, so a slow stdout reader no longer holds the HTTP response; when the queue (8 MiB) is full it answers 503 so the sender retries. Validation requests are answered whatever the queue state. On Ctrl+C or SIGTERM it stops admitting notifications (503) and writes out every one it already acknowledged before exiting; a second signal abandons them, and any acknowledged notification left unwritten, or lost to a closed stdout, is counted on stderr with exit code 1 (previously a closed stdout exited 0).
- `teams listen` limits bodies to 1 MiB (413), allows 10 seconds for headers and for the body (408), and serves at most 64 connections at once.
- `teams listen` no longer prints `clientState`.
- The default redirect URI is now `http://localhost/callback` on a free port instead of `http://localhost:8400/callback`, so a busy port 8400 no longer blocks login. App registrations with `http://localhost:8400/callback` keep working because the port is ignored.
- Updated Rust dependencies (`tokio` 1.54.1, `serde_json` 1.0.154, `uuid` 1.28.0, `webbrowser` 1.2.5) and the CI and release workflows' `Swatinem/rust-cache` pin to v2.9.3. `rust-version` in `Cargo.toml` is now 1.89, the oldest Rust the dependencies build with; it said 1.75.
- Updated `sha2` to 0.11.1 (it computes the PKCE code challenge for browser login). The challenge is now unit-tested against the RFC 7636 example, so a hashing or encoding change cannot slip through as failed logins.

### Fixed

- Log lines on stderr are coloured only when stderr is a terminal and `NO_COLOR` is unset, so `teams listen 2>listen.log` and other redirected logs no longer contain ANSI escape codes. `--no-color` still turns colour off.
- `teams listen` on a port already in use exits with code 10 (`CONFIG_ERROR`), naming the address and the OS error, instead of reporting an authentication failure (exit code 3).
- The browser login keeps serving its loopback redirect URI until the callback arrives. It served one connection, so a browser's speculative connection or a second request could leave it waiting forever or failing with "Failed to receive auth code from callback". A redirect carrying the identity platform's `error` (a declined consent, say) now fails the login at once with `error` and `error_description` (exit code 3) instead of that generic message, requests to other paths or with another `state` are ignored, and the login gives up after 10 minutes with no callback.
- Updated `rustls-webpki` to 0.103.16 and `webpki-roots` to 1.0.11 for RUSTSEC-2026-0382 and RUSTSEC-2026-0381, under which certificates issued by distrusted intermediates could still be accepted. The `tokio` update also clears RUSTSEC-2026-0386 (unsound `AsyncFd` constructors).
- `teams subscribe list` failed with HTTP 400 ("Query option 'Top' is not allowed"), because Graph rejects `$top` on `/subscriptions`. It now pages by `@odata.nextLink` alone, as `channel list` and `chat members list` do, and ignores `--page-size`.

## v0.10.0 - 2026-10-15

### Added

- List commands without `--all-pages` report whether the result is complete: JSON `metadata.has_more`, plus `metadata.next_link` (Graph's `@odata.nextLink`) when more pages exist. Previously a truncated page looked identical to a complete collection. With `--all-pages`, `has_more` is false. `data` is unchanged. Human and plain output print "More results available; use --all-pages" on stderr.
- `teams message attachments delete (--chat C | --team T --channel CH [--reply R]) --message M [--index N] --yes` deletes files attached with `message send --attach` from the sender's OneDrive. Each `reference` attachment is resolved through Graph's shares API and deleted only when it is in the caller's own drive; inline images, other people's files and channel files are skipped with a reason. A file already gone counts as deleted. The message and its file card are left as they are; `message delete` is separate.

### Changed

- `Retry-After` is read in its HTTP-date form as well as seconds, honored on retried 5xx responses as well as 429, and capped at 300 seconds.
- Updated Rust dependencies (`dirs` 7.0.0, `comfy-table` 8.0.1, `hyper` 1.12.0, `tokio` 1.53.2, `uuid` 1.27.0, and `want` 0.3.2) and the release workflow's artifact upload/download actions.

### Fixed

- Updated `rustls` to 0.23.46 and `quinn-proto` to 0.11.18 to resolve nine RustSec advisories (RUSTSEC-2026-0338 through RUSTSEC-2026-0342 and RUSTSEC-2026-0346 through RUSTSEC-2026-0349).
- `teams team update` and `teams channel update` no longer fail with "Failed to parse API response" (exit 1) after a successful update. Graph answers these with 204 No Content; every PATCH-based update now reads the resource back when the response has no body, so the output is unchanged. If that read fails, the error says the update succeeded (exit 1).
- A POST is no longer replayed after a 5xx or a network failure once sent, which could post a message twice. It fails with exit code 8 or 7 and a hint to check before re-sending. A 429 or a connection that was never made is still retried, and read-only POSTs (`search`, `presence get-batch`) retry as before.
- A POST answered 2xx whose response body cannot be read or parsed now says Graph performed it and not to re-send, instead of a bare network or parse error.
- `teams auth status` reports `user` and `tenant_id` at the top level, as `auth list` does, not only inside `token_diagnostics`. A token store that cannot be read is now `KEYRING_ERROR` (exit code 10) instead of `authenticated: false`, and a signed-out profile says why in `reason`.
- A token store that cannot be read (a locked keychain, a denied prompt, an SSH session) is reported as `KEYRING_ERROR` (exit code 10) with its cause, instead of "Not authenticated" (exit code 3). Only a profile with no stored token is "Not authenticated".
- A profile index that cannot be read or parsed is an error (exit code 10) instead of an empty list. `auth login` no longer rewrites it to hold only the new profile, orphaning the rest; `auth list` and `auth logout --all` no longer report no profiles as a success.
- `teams auth switch` with `--config` wrote the selected profile to the default config file, overwriting it with the `--config` file's contents and leaving that file unchanged. It now saves to the `--config` file, and a failed save is a config error (exit code 10) instead of a warning beside a success.
- `teams config path` ignored `--config` and always reported the default path. It now reports the file in use, with `source` set to `flag` or `default`.
- Config saves (`config set`, `config init`, `auth switch`) truncated the file and wrote it in place, so a failed write could leave a partial config. They now write a temporary file and rename it over the original, as `auth login` already did.
- A new profile's name is limited to 1-64 of `A-Z a-z 0-9 . _ -`, not `.` or `..`. `TEAMS_CLI_TOKEN_STORE=file` maps `:`, `/` and `\` to `.` in token file names, so `customer/a` and `customer.a` shared one token and could run as the wrong account. `auth login`, `config set profiles.<name>...` and `config set default.profile` refuse another name as invalid input (exit code 2). Profiles already in the config or the token index keep working under their names. A new name whose token file name matches an existing profile's, ignoring case, is refused too.
- Attachment deletion also rejects `remoteItem` references, including local shortcuts into another drive.
- `message attachments download` and `file download --path` write through a uniquely named, exclusively created temp file in the target directory, then rename it into place. Attachments used a fixed `<target>.part`, so a symlink there redirected the write, a stray `.part` was clobbered, and concurrent downloads to one target shared a temp file; `file download` wrote in place, through any symlink at the target, and left a truncated file on failure; a symlink at the target is now replaced. A failed write now leaves no temp file. An existing target is still overwritten and keeps its mode, and `message attachments download --path -` still streams to stdout.

## v0.9.0 - 2026-10-04

### Added

- `teams --version` names the storage namespace of any build that does not use the release namespace `teams-cli`, and `teams config path` reports `namespace` in every build.
- A delegated `teams auth login --client-id X --tenant-id Y` (browser or device code) saves both IDs to the profile's `client_id` and `tenant_id` in the config file once the login succeeds, so the next login for that profile signs in through the same app registration instead of falling back to the built-in app. Only those two keys are edited, so comments and other settings survive, and the file is replaced atomically. Saving is best effort: if the write fails, the login still succeeds with a warning. `TEAMS_CLI_CLIENT_ID` and `TEAMS_CLI_TENANT_ID` still apply but are not saved, and a client credentials login saves nothing, so its confidential app never becomes the profile's delegated one. Login prints the application and tenant it signs in through, and where each came from, on standard error, and its output includes `client_id`, `tenant_id` and `saved_to_config`. The client secret is never saved.

### Changed

- Debug builds from source keep their tokens under the keyring service `teams-cli-dev` and their config in a `teams-cli-dev` directory, so by default they no longer read or rewrite an installed release's keychain items or config file. Release builds keep `teams-cli`, and existing installs need no action. `TEAMS_CLI_BUILD_NAMESPACE`, set at compile time, chooses another namespace. A developer who signed in with a debug build before this change signs in once more, and a command that finds no token in such a build names the namespace it looked in.

### Fixed

- `teams auth login --help` no longer prints the value of `TEAMS_CLI_CLIENT_SECRET` when it is set. The help still names the variable, and now recommends it over `--client-secret`, whose value shows in process listings and shell history.

## v0.8.0 - 2026-09-29

### Added

- `teams message undelete` restores a message removed with `message delete`, through the Graph `undoSoftDelete` action.
- `message list` and `message get` include `deletedDateTime` on soft-deleted messages.
- `teams chat list --order-by activity` lists the chats with the newest message first and includes that message's preview on each row (`lastMessagePreview`: id, `createdDateTime`, `messageType`, `body`, `from`), so a caller watching many chats can tell from one page which of them have something new instead of reading every chat in turn. The default listing gives no such signal: Graph's order is not by activity, and `lastUpdatedDateTime` moves on renames and membership changes rather than messages — a chat last "updated" in June carried a message from the same morning (observed 2026-09-08), and a poller keyed on it read a tag nine minutes late. Graph orders `/me/chats` by `lastMessagePreview/createdDateTime` only, descending only, and returns the preview only when it is expanded, so the one flag sets both `$expand` and `$orderby`. The human table shows the newest message's time and sender in place of the update time; JSON carries the preview object and omits it entirely from a plain listing.
- `TEAMS_CLI_TOKEN_STORE=file` keeps tokens in files under the config directory's `tokens/` instead of the OS keyring — one file per entry, the directory `0700` and each file `0600` on Unix, written through a temporary file that is flushed to disk and renamed so a reader never sees a partial token. It exists for a process that cannot answer a keychain dialog or has no keyring: a daemon, a server, a container. On macOS the keychain grants access to a code signature, so an unattended process running a freshly built binary is asked once per profile per build; signing the build with a local identity does not end that, because the keychain item's partition list names build hashes (measured 2026-09-09, three dialogs in one day for one daemon); and on Linux without a Secret Service the keyring cannot store a token at all. The keyring stays the default and the two stores are independent. `list_profiles` and the profile index now go through the same store abstraction as the token, so `auth list` works under either. Any value other than `keyring`, `keychain` or `file` is refused before the command runs as invalid input (exit code 2), rather than reading as "not signed in", and a token file readable by other users draws a warning on stderr.
- `teams message send --chat C --quote MESSAGE_ID` quote-replies to a chat message the way the Teams client's Reply does, with the quoted message shown as a card above the new text. It uses Graph's v1.0 `replyWithQuote` action, which builds the quote card and its preview from the quoted message itself. `--quote` repeats to quote several messages of the chat at once (Graph accepts up to 5), and combines with `--mention`, `--image` and `--attach`. An ID that is not a message in the chat is reported as `NOT_FOUND` (exit code 5). Chats only.

### Changed

- Refreshed the Rust dependencies: the rust-minor group (`clap` 4.6.7, `clap_complete` 4.6.11, `toml` 1.1.6, `hyper` 1.11.1, `hyper-util` 0.1.21, `uuid` 1.26.1, `thiserror` 2.0.21) and `rustls` 0.23.37 → 0.23.45 (with `rustls-webpki` 0.103.15), which clears RUSTSEC-2026-0285 (TLS 1.3 handshake messages accepted across encryption-level boundaries; `rustls` is reached through `reqwest`) from `cargo audit`. The release workflow's `softprops/action-gh-release` pin moves to v3.0.3.
- `teams message delete` now calls the Graph `softDelete` action and accepts `--chat <chat-id>` as well as the `--team`/`--channel` pair, plus `--reply <reply-id>` for channel thread replies. Deletion must be confirmed with `--yes`; without it the command exits with code 2 and sends nothing. On success the message is read back so the output shows `deletedDateTime`. Channel targets need the `ChannelMessage.ReadWrite` delegated scope, which requires admin consent; a 403 on a channel target without it names the scope and `teams auth consent-url`.

### Fixed

- `message list` and `message get` no longer drop a mention's identity when the mention is not a person. The mention model only knew the `user` form of Graph's `chatMessageMentionedIdentitySet`, so an @Everyone (a `conversation` identity naming the chat or channel itself), an @channel or @team, and a team `tag` mention all read back as `"mentioned": {}` on every output, making a resolved mention look like one that never resolved. A bot or other app mentioned by a person (an `application` identity) was dropped the same way, and so was the sender of a message a bot, workflow or connector posted: `from` read back as `{}` and the human list's From column was blank. Every identity form Graph documents — `user`, `application`, `device`, `conversation` and `tag` — is now retained with its id, display name and type; the From column falls back to the application's name; user mentions and the send-side `--mention` shape are unchanged.
- `teams message delete` previously sent the DELETE verb, which Microsoft Graph rejects for messages ("Requested API is not supported"), so the command could never delete anything.

## v0.7.0 - 2026-09-06

### Added

- `teams message send --subject TEXT` sets the subject line on a channel root message — the bold title Teams renders above the body, the same field the client offers behind "Add a subject". Channel sends only: chat messages have no subject, so `--subject` with `--chat` (or without `--channel`) is rejected as invalid input before anything is sent.
- `teams message list --team T --channel C --message-id ROOT` lists the replies under one channel thread root, paged like any other listing. Without it a caller could not tell from the existing channel-list output whether a question had already been answered, because that listing returns thread roots only.
- `teams message reply --mention USER` (repeatable) tags a person in a threaded reply the same way `message send --mention` does, so a name in a reply notifies rather than merely appears.
- `teams presence set-preferred --availability A [--expiration D]` and `teams presence clear-preferred` manage the user-preferred presence, the layer Microsoft Graph ranks above every presence session while one exists. The Teams client's "Appear offline" lives there as `Offline`/`OffWork`, and until now nothing in the CLI could reach it: `presence set` returned success and the account stayed offline. Each of the six availabilities Graph accepts here has exactly one activity, so the command derives it and reports both back. The expiration is checked as a positive ISO 8601 duration in whole day, hour, minute and second units but not bounded, because Graph documents defaults of one day for `Busy` and `DoNotDisturb` and seven days otherwise, rather than a range.

### Changed

- Refreshed two Rust dependencies: `uuid` 1.24.1 → 1.26.0 (the weekly rust-minor group) and `rand` 0.9.2 → 0.9.3, which clears RUSTSEC-2026-0097 (`rand::rng()` unsound with a custom logger; reached only through `reqwest`'s QUIC dependency) from `cargo audit`.

### Fixed

- Windows builds reserve an 8 MiB main-thread stack, matching Linux and macOS. Windows gives the main thread 1 MiB by default, and building clap's command tree for this many subcommands needs almost all of it in an unoptimized build, so any addition to the `message` command made every debug and test invocation of `teams` on Windows — `--help` included — fail with `thread 'main' has overflowed its stack`, and `cargo test` failed on `windows-latest` while passing on Linux and macOS. A build script now passes `/STACK:8388608` to the MSVC linker (`--stack` on the GNU toolchain). The reservation is address space rather than committed memory, so an idle process costs nothing extra.
- Plain lists retain optional fields that first appear after the first row, including message subjects. Human message lists include a Subject column; JSON still omits absent subjects.
- `message list` and `message get` no longer drop the `subject` of a message. The `ChatMessage` model had no `subject` field, so a channel root message's subject — returned by Graph on both reads — silently vanished from every output: a message posted with a subject read back without one. Messages without a subject are unchanged and gain no `"subject": null` noise.
- `teams message send --chat … --attach FILE` now shares each uploaded file with the chat's other members. The upload lands in the sender's OneDrive (`Microsoft Teams Chat Files`), where nobody else has access; the Teams client grants every member read permission when it attaches a file, but the CLI did not, so recipients got "you don't have permission" when they opened the attachment. After each upload the CLI now grants the members read access through the drive item's `invite` action, with no notification email. A member is addressed by Entra object ID only when the roster shows the same tenant as the sender — Graph documents that a chat's membership can span tenants, and an object ID means nothing outside its own directory — and by email otherwise; the sender is skipped. Every step is best-effort: a failed member lookup, a member with no usable address, or a refused grant warns on stderr and says to share the file from OneDrive by hand, and the upload and the message still go through. Channel attachments are unchanged: they live in the team's SharePoint library, which channel members already read.

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
