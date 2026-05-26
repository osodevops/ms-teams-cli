# 5-minute local quickstart

## 1. Install or build

From source:

```bash
cargo build
```

Run the local binary:

```bash
target/debug/teams --help
```

Installed binary:

```bash
teams --help
```

## 2. Sign in

Device-code flow is the most reliable first test:

```bash
teams auth login --device-code
```

Check auth:

```bash
teams auth doctor --output json
```

## 3. Read your Teams context

```bash
teams user me --output json
teams team list --output json
teams chat list --page-size 10 --output json
```

## 4. Read recent messages from a chat

```bash
CHAT_ID=$(teams chat list --output json | jq -r '.data[0].id')
teams message list --chat "$CHAT_ID" --page-size 5 --output json
```

If one chat returns `403`, try another chat. Meeting rosters can make individual chats inaccessible.

## 5. Send only to a safe test target

Create or choose a dedicated test chat/channel first.

```bash
teams message send --chat "$CHAT_ID" --body "teams-cli local smoke test" --output json
```

Do not send test messages into client or production chats.
