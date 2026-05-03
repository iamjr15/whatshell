# Whatshell Troubleshooting

## Authentication Failures

Run:

```bash
whatshell auth status --json
whatshell doctor --json
```

If the session is missing or logged out, ask the user to run `whatshell auth` and scan the QR code from WhatsApp Linked Devices.

## Store Locks

Whatshell uses a process lock around live WhatsApp session commands. If a command reports that the store is locked, another Whatshell process is using the same store.

Do this:

```bash
whatshell doctor --json
```

Then wait for the active command to finish, stop the other process if it is stuck, or use a separate `--store <path>` for a separate account or isolated test profile. Do not run live Whatshell commands in parallel against the same store.

## Contact Lookup

Do not search local files to infer contacts. Use:

```bash
whatshell contacts sync --json
whatshell contacts search "<query>" --json
whatshell contacts check +15551234567 --json
```

If `contacts list` or `contacts search` is stale, run `contacts sync` without `--offline`.

## Index Freshness

Whatshell reads chats, contacts, and messages from its local WhatsApp-derived index. If expected chats, contacts, media, or messages are missing, refresh the index before concluding the data is unavailable:

```bash
whatshell sync --once --json
whatshell chats list --json
whatshell messages search "<query>" --json
```

For contacts:

```bash
whatshell contacts sync --json
whatshell contacts search "<query>" --json
```

Then use `messages context` before drafting a reply.

## Send Failures

Check these common causes:

- The account is not authenticated.
- Another command is holding the same store lock.
- The contact or group JID is invalid.
- Media paths are wrong or unreadable.
- The command timed out on a slow connection.
- WhatsApp rejected the request because the account, chat, or protocol path does not allow that operation.

Prefer a dry run where available. For live sends, preserve the command output and message ID.

## Safe Retry Pattern

For failed live operations, do not blindly retry destructive or public actions. First run:

```bash
whatshell doctor --json
whatshell auth status --json
```

Then verify the target with the narrowest read command, such as `contacts info`, `groups info`, `messages show`, or `messages search`. Retry only after the target, authentication state, and store lock state are clear.

## Unsupported WhatsApp Surfaces

Do not invent commands for newsletters/channels, communities, or group join-by-invite. They are intentionally absent because previous live protocol paths were unreliable.

## Privacy And Safety

- Avoid exposing full chat histories unless the user asked for them.
- Summarize instead of dumping private messages when possible.
- Confirm recipients before sending.
- Do not use Whatshell for spam, harassment, or unsolicited bulk messaging.
- Undo reversible test actions after verification when the user asks for live testing.
