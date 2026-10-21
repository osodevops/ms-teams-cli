---
name: teams-cli-admin
description: "Manage Microsoft Teams structure with the teams CLI: create, update, clone, archive, unarchive or delete a team; add or remove team and channel members; create, rename or delete channels; manage tags; list, install or uninstall team apps; list, add or remove channel tabs. Use when asked to create a team or channel, add someone to a team, archive a team, clone a team, tag a group, install a Teams app or add a tab."
---

# Teams CLI Admin

Create and maintain teams, channels, tags, apps and tabs. These commands change shared resources: read first, write second.

## Before you start

```bash
teams auth status --output json
```

Exit 1 or 3 means you are not signed in: use `teams-cli-setup`. Always pass `--output json` when parsing. Exit 4 usually means you are not an owner of the team or lack admin consent. Report it. Do not retry.

## Teams

```bash
teams team list --output json
teams team get <team-id> --output json
teams team create --name "Project Atlas" --description "Atlas delivery"
teams team update <team-id> --name "Project Atlas 2" --description "Updated"
teams team clone <team-id> --name "Atlas Template" --parts channels,tabs,settings --visibility private
teams team archive <team-id>
teams team unarchive <team-id>
teams team delete <team-id>
```

- `team create` and `team clone` may finish asynchronously. `team list` afterwards confirms it exists.
- `clone --parts` takes a comma-separated subset of `apps,tabs,settings,channels,members`. The default is all five, so say what you drop.
- `archive` makes the team read-only. `unarchive` reverses it. `delete` removes the team and its Microsoft 365 group; only an admin can restore it, and only for a limited time.

## Team members

```bash
teams team members list <team-id> --output json
teams team members add <team-id> --user-id <user-id> --role member
teams team members remove <team-id> <membership-id>
```

`remove` takes the membership id from `members list`, not the user id. Find a user id with `teams user resolve "<user-email>"`. Check the roster before and after.

## Channels

```bash
teams channel list <team-id> --output json
teams channel create <team-id> --name "Releases" --description "Release notes" --membership-type standard
teams channel update <team-id> <channel-id> --name "Releases 2026"
teams channel delete <team-id> <channel-id>
teams channel members list <team-id> <channel-id> --output json
teams channel members add <team-id> <channel-id> --user-id <user-id> --role member
teams channel members remove <team-id> <channel-id> <membership-id>
```

`--membership-type` is `standard`, `private` or `shared`. Channel member commands apply to private and shared channels.

## Tags

```bash
teams tag list <team-id> --output json
teams tag create <team-id> --name "oncall" --members <user-id>,<user-id>
teams tag update <team-id> <tag-id> --name "oncall-eu"
teams tag add-member <team-id> <tag-id> --user <user-id>
teams tag remove-member <team-id> <tag-id> --user <user-id>
teams tag delete <team-id> <tag-id>
```

## Apps and tabs

```bash
teams app list <team-id> --output json
teams app install <team-id> --app-id <catalog-app-id>
teams app uninstall <team-id> --app-id <installation-id>
teams tab list <team-id> <channel-id> --output json
teams tab create <team-id> <channel-id> --app-id <catalog-app-id> --name "Board" --content-url <https-url>
teams tab delete <team-id> <channel-id> --tab-id <tab-id>
```

- `app install` takes the catalog app id. `app uninstall --app-id` takes the installation id from `app list`, a different value.
- There is no tab update command. Delete and re-create the tab.

## Destructive commands

These cannot be undone, or are disruptive: `team delete`, `team archive`, `team unarchive`, `channel delete`, `app uninstall`, `tag delete`, `tab delete`, and any `members remove`.

1. Read the target first (`team get`, `channel get <team-id> <channel-id>`, `tag get`, `app list`) and show the user its name and id.
2. Ask for explicit confirmation naming that exact target, for example "Delete channel Releases (<channel-id>) in team Project Atlas (<team-id>)?".
3. Run the single command only after a clear yes. Never loop destructive commands over a list without confirming the full list.
4. Read back (`team list`, `channel list`) to confirm the result.

None of the admin commands above has a `--yes` flag (only `message delete` and `message attachments delete` do; see `teams-cli-messaging`), so the user's confirmation is the only guard.

## Related skills

- `teams-cli-setup`: install, sign in, exit codes, output contract.
- `teams-cli-messaging`: read, send and reply in chats and channels.
- `teams-cli-automation`: presence, meetings, notifications, webhooks.
