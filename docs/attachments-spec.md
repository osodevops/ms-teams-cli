# Specification: Message Attachments and Inline Images (Read and Send)

Status: Part I (read) implemented; Part II (send) in progress
Date: 2026-07-08
Branch target: follow-up to `feat/profile-scopes`

Part I covers reading what other people put in messages; Part II covers sending.
The [attachments for humans](#attachments-for-humans-hosted-contents-vs-files-and-the-scopes-they-need)
section explains the two storage mechanisms and their OAuth scopes without assuming
familiarity with Teams internals — start there if `hostedContents` means nothing to you.

# Part I: Reading

## Problem

Users routinely paste screenshots into Teams messages. `teams message get` returns the
message body verbatim, but the screenshot itself is unreachable through the CLI. Agents
and scripts consuming the JSON envelope see an `<img>` tag they cannot fetch, because the
image bytes sit behind an authenticated Microsoft Graph endpoint.

Two distinct mechanisms are involved, and the CLI currently supports neither:

### 1. Inline images — `hostedContents` (the screenshot case)

When a user pastes or drags an image directly into the compose box, Teams stores it as a
`chatMessageHostedContent`. The message body HTML references it by an authenticated Graph
URL:

```html
<img src="https://graph.microsoft.com/v1.0/teams/{team-id}/channels/{channel-id}/messages/{message-id}/hostedContents/{hosted-content-id}/$value"
     width="410" height="204" alt="image" itemid="0-frca-d14-...">
```

Verified against a live message (team `5c42feaf`, channel `19:b8fe94da...@thread.skype`,
message `1783503421261`):

- `GET .../messages/{id}/hostedContents` lists items but returns `contentBytes: null` and
  `contentType: null` — metadata only. Bytes require a per-item call.
- `GET .../hostedContents/{hosted-content-id}/$value` returns the raw bytes
  (`200`, `image/png`, correct pixels) **using the token the CLI already holds**.
  `ChannelMessage.Read.All` (channels) / `Chat.ReadWrite` or `ChatMessage.Read` (chats)
  suffice. No new scopes required.
- The hosted-content ID is base64 and decodes to
  `id=x_...,type=1,url=https://eu-api.asm.skype.com/v1/objects/...` — an internal Skype
  asset pointer. Treat it as opaque; always fetch through Graph, never the ASM URL.

So the screenshot case is purely a missing-feature problem: the CLI never calls these
endpoints and gives callers no way to do so.

### 2. File attachments — `attachments[]` with `contentType: "reference"`

Files attached via the paperclip (or drag-dropped as files rather than pasted as pixels)
appear in the message's `attachments` array pointing at SharePoint/OneDrive:

```json
{
  "id": "C0F75B79-7D00-4DC9-918F-5FAEDD1086A4",
  "contentType": "reference",
  "contentUrl": "https://{tenant}-my.sharepoint.com/personal/{user}/Documents/Microsoft%20Teams%20Chat%20Files/NetskopeLogs.zip",
  "name": "NetskopeLogs.zip"
}
```

Two defects here:

- **The CLI silently drops `contentUrl`.** `ChatMessageAttachment` in
  `src/models/message.rs` has no `content_url` field, so even the pointer to the file is
  lost from `message get` output.
- **No download path.** The canonical way to download is Graph's sharing-URL resolver:
  base64url-encode the `contentUrl`, prefix `u!`, strip padding, then
  `GET /shares/{token}/driveItem/content`. Verified live: this returns **403
  `accessDenied` with current scopes** — it requires `Files.Read.All` (delegated).
  This is exactly the per-profile scope-upgrade case `feat/profile-scopes` was built for.

### Other attachment content types (in scope for parsing, out of scope for download)

- `application/vnd.microsoft.card.*` — adaptive/hero cards; `content` is inline JSON.
- `messageReference` — quoted replies; `content` is inline JSON.
- `application/vnd.microsoft.card.codesnippet` — code snippets; the attachment `content`
  JSON carries a `codeSnippetUrl` that resolves to a hostedContent, fetched via the same
  authenticated `/$value` mechanism as images. Worth including in the download path (it
  is text an agent wants to read), unlike cards.

## Prior art

Every serious open-source exporter/bridge converges on the same two-track approach this
spec proposes; none of them expose it as a composable CLI primitive, which is our gap to
fill.

- **[teams-chats-export](https://github.com/codeforkjeff/teams-chats-export)** (Python,
  Graph SDK) — the cleanest reference. Does not trust the `hostedContents` collection
  (bytes are always null there); instead regexes the body HTML for
  `src="...graph.microsoft.com/.../hostedContents/{id}/$value"` and GETs each with the
  bearer token. Also resolves code-snippet attachments to their hosted content. Handles
  per-item access failures with try/except. Delegated `Chat.Read`.
- **[matterbridge msteams bridge](https://github.com/42wim/matterbridge/pull/967)** (Go)
  — file attachments via the shares trick: `GetDriveItemByURL(contentUrl)` →
  `@microsoft.graph.downloadUrl` → plain HTTP download. Special-cases
  `application/vnd.microsoft.card.codesnippet` (parses `codeSnippetUrl` from attachment
  `content` JSON, fetches authenticated, wraps in a fenced code block). Never implemented
  inline pasted images.
- **[m365 CLI](https://github.com/pnp/cli-microsoft365)** (`m365 teams message get`) —
  prints raw message JSON only; downloading hosted content or resolving `reference`
  attachments is left entirely to the caller. This is where teams-cli sits today.
- **[Export-TeamsChat](https://github.com/mardahl/Export-TeamsChat)** (PowerShell) —
  downloads inline images to a sibling assets folder and rewrites the HTML `src` to local
  relative paths; explicitly does not download `reference` file attachments.
- **[teams-chat-backup](https://github.com/edgraaff/teams-chat-backup)** (Node) and
  similar exporters — paginate messages, save `image-#####` files, render local HTML.

Cross-project lessons the design below absorbs:

- The `<img src>` in body HTML is the source of truth for inline images; the
  `hostedContents` collection is only good for enumeration.
- The `/$value` response's `Content-Type` header is the only reliable MIME source for
  hosted contents; `driveItem.file.mimeType` is the equivalent for file references.
- The top failure modes in the wild are (a) reading `contentBytes` from the collection
  and getting nulls, (b) 404s from mis-encoding `contentUrl` (spaces, trailing slashes)
  before base64url, and (c) 403s from missing `Files.Read.All` silently swallowed by SDK
  wrappers. All three get explicit tests.
- Per-message reads (`GET .../messages/{id}`, `hostedContents/$value`, `/shares/...`)
  are ordinary non-metered Graph calls. Only the bulk export APIs (`getAllMessages`)
  ever required metered billing, and Microsoft removed that metering in August 2025 —
  irrelevant to this feature either way.
- No prior art exists for agent-native consumption (manifest of local paths + MIME
  types); every tool stops at "bytes on disk." The inventory/manifest design below is
  the differentiator.

## Design

### Goals

- An agent holding only the CLI can go from a message link/ID to readable image bytes on
  disk in one command.
- `message get` output becomes self-describing: attachments and inline images are
  enumerated with stable indices so callers can request downloads without parsing HTML.
- Follow existing patterns: JSON envelope, exit codes, `endpoints.rs` URL builders,
  `GraphClient::get_bytes`, `file download`'s `--path`/stdout convention.

### Non-goals

- Uploading inline images (send-side hostedContents) — Part II below.
- Rendering images in the terminal (sixel/kitty) — the consumer is usually an agent or a
  file; a path on disk is the right interface.
- OCR — leave interpretation to the caller.

### CLI surface

One new subcommand group plus one convenience flag.

#### `teams message attachments list`

```
teams message attachments list --team <TEAM> --channel <CHANNEL> <MESSAGE_ID>
teams message attachments list --chat <CHAT> <MESSAGE_ID>
```

Returns a unified inventory of everything downloadable in the message, merging both
mechanisms:

```json
{
  "success": true,
  "data": {
    "message_id": "1783503421261",
    "items": [
      {
        "index": 0,
        "kind": "hosted_content",
        "hosted_content_id": "aWQ9...",
        "content_type": "image/png",
        "alt": "image",
        "width": 410,
        "height": 204
      },
      {
        "index": 1,
        "kind": "file_reference",
        "attachment_id": "C0F75B79-...",
        "name": "NetskopeLogs.zip",
        "content_url": "https://...sharepoint.com/.../NetskopeLogs.zip"
      }
    ]
  }
}
```

`hosted_content` entries are discovered from `GET .../hostedContents` and enriched
(dimensions, alt text) by parsing `<img>` tags in the body HTML whose `src` matches the
hostedContents URL pattern. `content_type` for hosted contents is only known after a
byte fetch (the list endpoint returns null), so it is populated via sniffing on download
and omitted in `list` output if unknown.

#### `teams message attachments download`

```
teams message attachments download --team <TEAM> --channel <CHANNEL> <MESSAGE_ID> \
    [--index <N>] [--dir <DIR> | --path <FILE>]
```

- Default (no `--index`): downloads every downloadable item into `--dir` (default `.`),
  naming files `msg-{message_id}-{index}.{ext}` for hosted contents (extension from the
  `/$value` response's Content-Type header) and the attachment's sanitized `name` for
  file references (collision-suffixed).
- `--index N` downloads a single item; `--path FILE` (requires `--index`) sets an exact
  destination, and `--path -` streams bytes to stdout (mirrors `file download`).
- Hosted contents: `GET .../hostedContents/{id}/$value` via
  `GraphClient::get_bytes_with_content_type`.
- File references: encode `contentUrl` → `GET /shares/u!{b64url}/driveItem/content`
  (follows the 302 to a pre-authenticated download URL; MIME type from the response
  header).
- On success the JSON envelope reports one entry per item:
  `{index, kind, path, size, content_type}`. On any failure the command keeps
  downloading what it can, then exits with the first error's code (e.g. 4 on 403) and
  an error message naming the failed item and listing paths already saved.

#### `teams message get --with-attachments`

Convenience flag that inlines the `attachments list` inventory into the message envelope
under `data.attachment_items`, so agents can decide in one round-trip whether a download
pass is needed. Without the flag, `message get` output is unchanged **except** that the
attachment model now carries `contentUrl` (bug fix, additive).

### Implementation plan

Ordered, each step compiles and passes tests independently.

1. **Model fix** (`src/models/message.rs`): add `content_url`, `thumbnail_url`,
   `teams_app_id` to `ChatMessageAttachment`. Add `ChatMessageHostedContent` model
   (`id`, `contentBytes`, `contentType`). Unit-test round-trips.
2. **Endpoints** (`src/api/endpoints.rs`): hostedContents list + `$value` URL builders
   for channel, channel-reply, and chat scopes; `shares_drive_item_content(token)`; and
   a `sharing_url_token(url) -> String` helper (u! + base64url, no padding) with tests.
3. **API layer** (`src/api/messages.rs`): a `MessageRef` enum (channel / channel-reply /
   chat) with `get_message`, `list_hosted_contents`, `get_hosted_content_bytes`;
   `download_shared_item` in `src/api/files.rs`. Reuses the `GraphClient` retry
   machinery; reqwest follows the SharePoint 302 and strips the Authorization header
   cross-origin (same path the existing `file download` relies on).
4. **Inventory builder** (new `src/models/attachment_inventory.rs` or in `cli/message.rs`):
   pure function `build_inventory(&ChatMessage, hosted: &[ChatMessageHostedContent]) -> Vec<Item>`;
   parses body HTML for `<img src=".../hostedContents/{id}/$value">` to join dimensions
   and ordering. Use a lightweight regex on the known URL shape rather than an HTML
   parser dependency; unit-test against captured real payloads.
5. **CLI wiring** (`src/cli/message.rs`): `MessageCommand::Attachments { List, Download }`
   and the `--with-attachments` flag, following the `file download` handler's
   path/stdout/envelope conventions.
6. **MIME detection**: the `/$value` response's `Content-Type` header is the only
   reliable MIME source for hosted contents — add a
   `GraphClient::get_bytes_with_content_type` variant (or change `get_bytes` to return
   `(Vec<u8>, Option<String>)`) rather than sniffing magic bytes. For file references,
   take `driveItem.file.mimeType` from the `/shares/{token}/driveItem` resolution. Fall
   back to magic-byte sniffing only if the header is absent.
7. **Scope prerequisites**: hosted contents need nothing new. File references need
   `Files.Read.All` delegated — document in `docs/auth.md` and add to the recommended
   profile scope set so the `feat/profile-scopes` silent-upgrade flow picks it up. On
   403 from `/shares`, the error message must say: add `Files.Read.All` to the profile
   scopes and run `teams auth refresh`.
8. **Docs + skill**: update `docs/command-reference.md`, `docs/examples.md`, and the
   `teams-cli` agent skill so agents learn the pattern: `message get` → see
   `attachment_items` → `attachments download --dir`.
9. **Tests**: unit tests per step; integration tests (feature `integration`) that list
   and download hosted contents from a known message; a wiremock-style test for the 403
   scope-error message if the client tests already use a mock server (they do —
   `src/api/client.rs` test module).

### Error handling

- 403 on `/shares` → exit code 4 with the actionable scope message above.
- 404 on hostedContent (expired/deleted) → exit 5, name the index and message ID.
- Partial failures download what they can, then exit non-zero with an error that names
  the failed item and lists the paths already saved.
- Never write partial files: download to `{path}.part`, rename on success.

### Security notes

- Hosted-content bytes are fetched only via `graph.microsoft.com`; the embedded ASM
  (`asm.skype.com`) URLs inside decoded IDs are never contacted.
- Downloaded filenames derived from attachment `name` must be sanitized (strip path
  separators, `..`) before writing into `--dir`.
- Bearer token must not leak into logs at `-vvv`; reuse existing redaction.

## Resolved questions

- Code snippets: downloadable like images (`kind: "code_snippet"`, saved as `.txt`,
  declared `language` included in the inventory entry). Cards: their JSON is already
  inline in the message envelope; `list` surfaces them as `kind: "card"` with
  `downloadable: false`. Extracting image URLs from inside card JSON is left to
  callers — the heuristics are too fuzzy to bake in.
- Chat (1:1/group) parity: same `hostedContents` endpoints exist under
  `/chats/{id}/messages/{id}`; delegated `Chat.Read`/`Chat.ReadWrite` suffices (already
  in the default profile). Covered by integration tests.
- Reply messages: hosted contents also hang off
  `.../messages/{id}/replies/{reply-id}/hostedContents`; the endpoints module needs the
  reply variant since screenshots very often appear in thread replies.

# Attachments for humans: hosted contents vs. files, and the scopes they need

Teams has two completely different storage mechanisms hiding behind what looks like one
"add an image to my message" experience, and Microsoft's OAuth scopes track the storage,
not the user gesture. Understanding this split explains every scope requirement in this
feature.

## The two mechanisms

**Pasted screenshots ("hosted contents").** When you paste or drag an image *into the
text of a message*, the image does not become a file anywhere you can browse. Teams
stores the bytes as a blob glued to the message itself — Microsoft calls this a *hosted
content*. It lives and dies with the message, has no filename, and is reachable only
*through* the message. Because it travels inside the message, reading or sending it
needs only message permissions — no file permissions at all.

**Attached files ("reference attachments").** When you attach a file with the paperclip,
Teams uploads the file to real, browsable storage and the message merely carries a link:

- in a **channel**, the file lands in the team's SharePoint document library (the Files
  tab you can open in a browser);
- in a **chat**, the file lands in *the sender's own OneDrive*, in a folder called
  `Microsoft Teams Chat Files`.

The message's attachment entry is just `{name, contentUrl}` pointing at that storage.
So touching these files means touching SharePoint/OneDrive, and Microsoft treats "send
messages" and "read or write files in drives" as different doors with different keys.

## The scope table

Delegated scopes, per operation:

| Operation | CLI command | Message scope | File scope needed | Why |
|---|---|---|---|---|
| Read a pasted screenshot | `message attachments download` | `ChatMessage.Read` / `ChannelMessage.Read.All` | none | bytes come through the message (`hostedContents/$value`) |
| Send a pasted screenshot | `message send --image` | `ChatMessage.Send` / `ChannelMessage.Send` | none | bytes travel inside the message create call |
| Download an attached file | `message attachments download` | (same as reading the message) | `Files.Read.All` | the file lives in someone's OneDrive / a team's SharePoint; reading it is a drive read |
| Attach a file to a chat message | `message send --attach --chat` | `ChatMessage.Send` | `Files.ReadWrite` | the CLI must first upload the file into *your* OneDrive (`Microsoft Teams Chat Files`) |
| Attach a file to a channel message | `message send --attach --team/--channel` | `ChannelMessage.Send` | `Files.ReadWrite.All` | the CLI must first upload into the *team's* SharePoint library, which is not your drive — hence the broader `.All` |

Two consequences worth internalizing:

- `--image` works with the CLI's default scopes. If you only ever send screenshots, you
  never need a Files grant.
- `--attach` is really two operations — a drive upload followed by a message send — and
  the failure you hit without the Files scope happens at the *upload* step, before any
  message exists.

To add a scope: put it in the profile's `scopes` field in `config.toml`, then run
`teams auth refresh` (silent, no browser; see docs/auth.md). Scopes marked `.All` may
require a tenant admin's consent.

# Part II: Sending

## Problem

The read side (Part I) lets agents see screenshots and files in messages. The mirror
gesture — "send this screenshot / attach this file" — was still impossible: `message
send` supports only text/HTML bodies and adaptive cards, and `file upload` puts a file
in a channel's Files tab without attaching it to any message (and cannot touch chats).

## Design

### CLI surface

```
teams message send  (--team T --channel C | --chat CH) [--body TEXT | --stdin] \
    [--image PATH]... [--attach PATH]...
teams message reply --team T --channel C --message-id M [--body TEXT | --stdin] \
    [--image PATH]... [--attach PATH]...
```

- `--image PATH` (repeatable): sends the file as a *hosted content* — the pasted-
  screenshot experience. The message body gains an `<img>` per image. MIME type is
  guessed from the file extension and must be an image type.
- `--attach PATH` (repeatable): uploads the file to the correct drive for the target
  (sender's OneDrive `Microsoft Teams Chat Files` for chats, the channel's folder in
  the team's SharePoint library for channels), then references it from the message.
  Uploads use `@microsoft.graph.conflictBehavior=rename` so an existing file with the
  same name is never overwritten.
- `--body` becomes optional when at least one `--image`/`--attach` is given.
- Media forces the body content type to HTML; a plain-text `--body` is HTML-escaped
  and wrapped in `<p>` first.

### Graph mechanics

**Inline images** ride the message create call itself. The request gains a
`hostedContents` array; each item carries the base64 bytes and a temporary ID that the
body references *relatively*:

```json
{
  "body": {
    "contentType": "html",
    "content": "<p>look:</p><p><img src=\"../hostedContents/1/$value\"></p>"
  },
  "hostedContents": [{
    "@microsoft.graph.temporaryId": "1",
    "contentBytes": "iVBORw0KGgo...",
    "contentType": "image/png"
  }]
}
```

Graph rewrites the relative `src` into a permanent hosted-content URL on delivery.
Application (app-only) tokens cannot send hosted contents; the CLI already requires
delegated auth for all message mutation, so nothing changes.

**File attachments** are a two-step dance:

1. `PUT` the bytes into the right drive (chat → `/me/drive/root:/Microsoft Teams Chat
   Files/{name}:/content`, channel → the team drive folder that `filesFolder` reports,
   same as `file upload`). The response is a `driveItem`.
2. Send the message with an `attachments` entry whose `id` is **the GUID inside the
   driveItem's `eTag`** (e.g. `"{5FF69C5F-...},2"` → `5FF69C5F-...`), `contentType`
   `"reference"`, `contentUrl` = the driveItem's `webUrl`, `name` = its `name` — plus
   an `<attachment id="{guid}"></attachment>` tag in the body HTML, which is what makes
   the attachment card render in clients.

Simple upload caps at 4&nbsp;MB (the existing `MAX_UPLOAD_SIZE`); larger files need the
upload-session API, which is out of scope here — the CLI errors clearly instead.
Hosted contents ride a single JSON request, so images are capped at 3&nbsp;MB each to
stay under Graph's 4&nbsp;MB request limit after base64 expansion (+33%).

### Error UX (scope failures must teach)

A 403 — or a 404, since Graph masks drives the token cannot see as `itemNotFound`
rather than `accessDenied` (verified live: `GET /me/drive` without any Files scope
returns 404) — on the upload step returns the abbreviated version of the scope table
above, tailored to the target:

- chat: "Attaching files to chat messages uploads them to your OneDrive ('Microsoft
  Teams Chat Files') first — that needs the `Files.ReadWrite` delegated scope, which
  your token doesn't have. Inline images (`--image`) don't need it. Add the scope to
  your profile's `scopes` and run `teams auth refresh`."
- channel: same shape, naming the team's SharePoint library and `Files.ReadWrite.All`.

Both point at docs/auth.md for the long version.

### Implementation plan

1. **Models**: `SendMessageRequest` gains `hostedContents`; new `HostedContentUpload`
   (`@microsoft.graph.temporaryId`, `contentBytes`, `contentType`); `DriveItem` gains
   `eTag`.
2. **Endpoints**: chat-files upload URL under `/me/drive`, and a rename-on-conflict
   variant of the channel folder upload.
3. **API layer** (`src/api/files.rs`): `upload_chat_file`, `upload_channel_attachment`
   (rename semantics), both with the teaching 403 hints.
4. **CLI** (`src/cli/message_media.rs`): pure builders for the img/attachment HTML and
   the eTag-GUID extraction (unit-tested), plus the async assembly that reads files,
   guesses MIME types, enforces size caps, uploads, and extends `SendMessageRequest`.
5. **Docs**: command-reference, examples, auth.md scope list, man page, CHANGELOG.
6. **Verification**: live `--image` send with default scopes (must succeed); live
   `--attach` without a Files grant (must fail with the teaching hint); unit tests for
   builders and escaping.
