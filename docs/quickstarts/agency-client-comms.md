# Agency client communications quickstart

This guide is for agencies and consultancies that use Teams to communicate with clients.

## 1. Choose the auth model

Default:

```bash
teams auth login --device-code
```

Locked-down client tenant:

```bash
teams auth login --device-code \
  --client-id <client-approved-app-id> \
  --tenant-id <client-tenant-id>
```

Run diagnostics:

```bash
teams auth doctor --output json
```

## 2. Find the client chat

```bash
teams chat list --page-size 20 --output json
```

Store the chosen chat ID in your project automation secret store or local config.

## 3. Read safely

```bash
teams message list --chat "$CLIENT_CHAT_ID" --page-size 10 --output json |
  jq '.data[] | {id, createdDateTime, messageType, bodyChars: (.body.content | length)}'
```

This proves access without leaking client text into logs.

## 4. Post an update

```bash
cat <<'TEXT' | teams message send --chat "$CLIENT_CHAT_ID" --stdin --output json
Daily update:
- Deployment completed
- Smoke tests passed
- No client action required
TEXT
```

## 5. Operating rules

- Use a named profile per client.
- Keep customer tenant IDs and chat IDs out of public repos.
- Do not paste raw client messages into bug reports.
- Use a dedicated test chat before first production use.
- Confirm the consent screen shows the expected app and publisher before approval.
