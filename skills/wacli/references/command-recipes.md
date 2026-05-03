# WACLI Command Recipes

Use these recipes when the main skill instructions are not enough.

## Authentication

```bash
wacli auth
wacli auth --phone +15551234567
wacli auth status --json
wacli auth logout
```

`auth logout` removes local session files. The user must revoke a linked device from WhatsApp itself if they want to remove it remotely.

## Sync And Listen

```bash
wacli sync --once --json
wacli sync --follow --stream-jsonl
wacli listen --stream-jsonl
```

Use `sync --once` before reading the local index. Use `listen --stream-jsonl` only when the user wants live events.

## Chats And Messages

```bash
wacli chats list --json
wacli chats list --query "<name-or-phone>" --json
wacli messages list --chat <chat-or-phone> --limit 50 --json
wacli messages show --chat <chat-or-phone> --id <message-id> --json
wacli messages search "<query>" --limit 25 --json
wacli messages context --chat <chat-or-phone> --id <message-id> --before 5 --after 5 --json
```

Use `messages context` before replying to an old message so the response is grounded in the surrounding conversation.

## Sending

```bash
wacli send text --to <chat-or-phone> --message "<text>" --json
wacli send file --to <chat-or-phone> --file ./file.pdf --caption "<caption>" --json
wacli send react --to <chat-or-phone> --id <message-id> --reaction "+1" --json
wacli send poll --to <chat-or-phone> --question "<question>" --option Yes --option No --json
wacli send location --to <chat-or-phone> --latitude 12.9716 --longitude 77.5946 --name "Bengaluru" --json
wacli send contact --to <chat-or-phone> --name "<name>" --phone +15551234567 --json
```

When supported by the subcommand, use `--dry-run --json` before the live command.

## Replies And Message Actions

```bash
wacli messages reply --chat <chat-or-phone> --id <message-id> --message "<text>" --json
wacli messages edit --chat <chat-or-phone> --id <message-id> --message "<new-text>" --json
wacli messages revoke --chat <chat-or-phone> --id <message-id> --json
wacli messages delete-for-me --chat <chat-or-phone> --id <message-id> --json
wacli messages star --chat <chat-or-phone> --id <message-id> --json
wacli messages unstar --chat <chat-or-phone> --id <message-id> --json
```

Only edit, revoke, or delete messages when the user explicitly asks.

## Contacts

```bash
wacli contacts sync --json
wacli contacts list --json
wacli contacts search "<query>" --json
wacli contacts check +15551234567 --json
wacli contacts info +15551234567 --json
```

Prefer these commands over local address-book guessing. `contacts check` and `contacts info` use live WhatsApp lookups.

## Groups

```bash
wacli groups list --json
wacli groups info --jid <group-jid> --json
wacli groups invite-link --jid <group-jid> --json
wacli groups invite-link --jid <group-jid> --reset --json
wacli groups invite-info <invite-code> --json
wacli groups create --subject "<subject>" --participant <phone-or-jid> --dry-run --json
wacli groups set-subject --jid <group-jid> --subject "<subject>" --json
wacli groups set-description --jid <group-jid> --description "<description>" --json
wacli groups set-description --jid <group-jid> --clear --json
wacli groups add --jid <group-jid> <phone-or-jid> --json
wacli groups remove --jid <group-jid> <phone-or-jid> --json
wacli groups promote --jid <group-jid> <phone-or-jid> --json
wacli groups demote --jid <group-jid> <phone-or-jid> --json
wacli groups lock --jid <group-jid> --json
wacli groups unlock --jid <group-jid> --json
wacli groups announce --jid <group-jid> --json
wacli groups unannounce --jid <group-jid> --json
wacli groups ephemeral --jid <group-jid> --seconds 86400 --json
wacli groups approval --jid <group-jid> --on --json
wacli groups approval --jid <group-jid> --off --json
wacli groups member-add --jid <group-jid> --admins-only --json
wacli groups member-add --jid <group-jid> --everyone --json
wacli groups requests --jid <group-jid> --json
wacli groups leave --jid <group-jid> --json
```

Group mutations are highly visible. Confirm the target group and participants before running live commands.

## Presence, Profile, Blocking, Status

```bash
wacli presence online --json
wacli presence offline --json
wacli presence typing --chat <chat-or-phone> --json
wacli profile set-name "<name>" --json
wacli profile set-about "<about>" --json
wacli profile set-picture ./avatar.jpg --json
wacli profile remove-picture --json
wacli blocking list --json
wacli blocking block <contact> --json
wacli blocking unblock <contact> --json
wacli blocking is-blocked <contact> --json
wacli status text --message "<text>" --to <recipient> --json
wacli status image --file ./image.jpg --to <recipient> --caption "<caption>" --json
wacli status video --file ./video.mp4 --to <recipient> --caption "<caption>" --json
wacli status revoke --id <status-message-id> --to <recipient> --json
```

Treat these as side-effecting user-account actions.

## Export

```bash
wacli export messages --chat <chat-or-phone> --format jsonl --output ./chat.jsonl
wacli export analytics --json
```

Use exports when the user asks for summaries, audits, or offline processing of larger chat histories.
