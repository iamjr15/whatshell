---
name: whatshell
description: Uses Whatshell, the WhatsApp Web CLI, to safely read, search, summarize, send, reply to, and manage WhatsApp chats from coding agents. Use when a user asks to work with WhatsApp, WhatsApp Web, messages, chats, contacts, groups, media, status, presence, or replies through the terminal.
license: MIT
compatibility: Requires the `whatshell` command from the npm package or Cargo install. Works with Claude Code, Codex, OpenCode, and agents that support Agent Skills.
metadata:
  package: whatshell
  category: communications
---

# Whatshell

Use Whatshell when the user wants WhatsApp Web actions from the terminal. Prefer JSON output, keep actions reversible where possible, and treat sending, deleting, blocking, profile, status, and group changes as side-effecting operations.

## First Checks

1. Verify the command is available:

```bash
whatshell --version
```

2. Check local installation, session, and index status:

```bash
whatshell doctor --json
whatshell auth status --json
```

3. If the user has not authenticated, ask them to run:

```bash
whatshell auth
```

They must scan the QR code in WhatsApp under Settings > Linked Devices > Link a Device.

## Operating Rules

- Use `--json` for commands whose output you need to parse.
- Use `--store <path>` when the user provides a specific Whatshell store, test profile, or isolated account.
- Run live Whatshell commands serially per store. Do not parallelize sync, send, contact sync, group, status, profile, blocking, or chat mutation commands against the same store.
- Do not guess contacts from local files. Use Whatshell contact and chat commands.
- Prefer `whatshell contacts sync` before contact lookup unless the user explicitly asks for offline-only index reads.
- Prefer `whatshell sync --once --json` before reading recent chats or messages unless the user intentionally wants the existing local index.
- Do not send messages, change groups, delete messages, block contacts, update profile/status, or mark messages read unless the user explicitly requested that action or confirmed a proposed action.
- For destructive or public actions, show the exact command you plan to run unless the user has already given a direct instruction.
- Avoid spam, bulk unsolicited outreach, and anything that violates WhatsApp terms or the user's privacy expectations.
- Preserve message IDs, chat JIDs, group JIDs, status IDs, and invite links returned by commands when later cleanup, reply, revoke, or verification may be needed.
- If an operation fails, inspect the error, run `whatshell doctor --json`, and check whether the command needs authentication, a synced index, a live connection, a valid JID, serialized store access, or a supported WhatsApp protocol path.

## Common Workflows

Read and index recent activity:

```bash
whatshell sync --once --json
whatshell chats list --json
whatshell messages list --chat <chat-or-phone> --limit 20 --json
```

Find people and chats through WhatsApp-derived data:

```bash
whatshell contacts sync --json
whatshell contacts search "<name-or-phone-fragment>" --json
whatshell contacts check +15551234567 --json
whatshell contacts info +15551234567 --json
```

Search messages and inspect context:

```bash
whatshell messages search "<query>" --limit 20 --json
whatshell messages context --chat <chat-or-phone> --id <message-id> --before 5 --after 5 --json
```

Send or reply:

```bash
whatshell send text --to <chat-or-phone> --message "<text>" --json
whatshell messages reply --chat <chat-or-phone> --id <message-id> --message "<text>" --json
```

Use dry runs before side effects when supported:

```bash
whatshell send text --to <chat-or-phone> --message "<text>" --dry-run --json
whatshell groups create --subject "<name>" --participant <phone-or-jid> --dry-run --json
```

## Supported Surface

Whatshell supports authentication, sync/listen, chat and message search, message context, replies, text/media sends, reactions, polls, locations, contact cards, media listing/download, chat archive/pin/mute/read/delete, contact sync/check/info, group listing and administration, presence, profile changes, blocking, status/story posts, exports, and diagnostics.

Do not try newsletter/channel commands, community commands, or group join-by-invite commands. They are intentionally not exposed until those WhatsApp Web protocol paths are reliable.

## References

- For command recipes, read `references/command-recipes.md`.
- For troubleshooting and safe operation, read `references/troubleshooting.md`.
