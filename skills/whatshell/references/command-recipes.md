# Whatshell Command Recipes

Use these recipes when the main skill instructions are not enough.

## Authentication

```bash
whatshell auth
whatshell auth --phone +15551234567
whatshell auth status --json
whatshell auth logout
```

`auth logout` removes local session files. The user must revoke a linked device from WhatsApp itself if they want to remove it remotely.

## Sync And Listen

```bash
whatshell sync --once --json
whatshell sync --follow --stream-jsonl
whatshell listen --stream-jsonl
```

Use `sync --once` before reading the local index. Use `listen --stream-jsonl` only when the user wants live events.

## Chats And Messages

```bash
whatshell chats list --json
whatshell chats list --query "<name-or-phone>" --json
whatshell messages list --chat <chat-or-phone> --limit 50 --json
whatshell messages show --chat <chat-or-phone> --id <message-id> --json
whatshell messages search "<query>" --limit 25 --json
whatshell messages context --chat <chat-or-phone> --id <message-id> --before 5 --after 5 --json
```

Use `messages context` before replying to an old message so the response is grounded in the surrounding conversation.

## Sending

```bash
whatshell send text --to <chat-or-phone> --message "<text>" --json
whatshell send file --to <chat-or-phone> --file ./file.pdf --caption "<caption>" --json
whatshell send react --to <chat-or-phone> --id <message-id> --reaction "+1" --json
whatshell send poll --to <chat-or-phone> --question "<question>" --option Yes --option No --json
whatshell send location --to <chat-or-phone> --latitude 12.9716 --longitude 77.5946 --name "Bengaluru" --json
whatshell send contact --to <chat-or-phone> --name "<name>" --phone +15551234567 --json
```

When supported by the subcommand, use `--dry-run --json` before the live command.

## Replies And Message Actions

```bash
whatshell messages reply --chat <chat-or-phone> --id <message-id> --message "<text>" --json
whatshell messages edit --chat <chat-or-phone> --id <message-id> --message "<new-text>" --json
whatshell messages revoke --chat <chat-or-phone> --id <message-id> --json
whatshell messages delete-for-me --chat <chat-or-phone> --id <message-id> --json
whatshell messages star --chat <chat-or-phone> --id <message-id> --json
whatshell messages unstar --chat <chat-or-phone> --id <message-id> --json
```

Only edit, revoke, or delete messages when the user explicitly asks.

## Contacts

```bash
whatshell contacts sync --json
whatshell contacts list --json
whatshell contacts search "<query>" --json
whatshell contacts check +15551234567 --json
whatshell contacts info +15551234567 --json
```

Prefer these commands over local address-book guessing. `contacts check` and `contacts info` use live WhatsApp lookups.

## Groups

```bash
whatshell groups list --json
whatshell groups info --jid <group-jid> --json
whatshell groups invite-link --jid <group-jid> --json
whatshell groups invite-link --jid <group-jid> --reset --json
whatshell groups invite-info <invite-code> --json
whatshell groups create --subject "<subject>" --participant <phone-or-jid> --dry-run --json
whatshell groups set-subject --jid <group-jid> --subject "<subject>" --json
whatshell groups set-description --jid <group-jid> --description "<description>" --json
whatshell groups set-description --jid <group-jid> --clear --json
whatshell groups add --jid <group-jid> <phone-or-jid> --json
whatshell groups remove --jid <group-jid> <phone-or-jid> --json
whatshell groups promote --jid <group-jid> <phone-or-jid> --json
whatshell groups demote --jid <group-jid> <phone-or-jid> --json
whatshell groups lock --jid <group-jid> --json
whatshell groups unlock --jid <group-jid> --json
whatshell groups announce --jid <group-jid> --json
whatshell groups unannounce --jid <group-jid> --json
whatshell groups ephemeral --jid <group-jid> --seconds 86400 --json
whatshell groups approval --jid <group-jid> --on --json
whatshell groups approval --jid <group-jid> --off --json
whatshell groups member-add --jid <group-jid> --admins-only --json
whatshell groups member-add --jid <group-jid> --everyone --json
whatshell groups requests --jid <group-jid> --json
whatshell groups leave --jid <group-jid> --json
```

Group mutations are highly visible. Confirm the target group and participants before running live commands.

## Presence, Profile, Blocking, Status

```bash
whatshell presence online --json
whatshell presence offline --json
whatshell presence typing --chat <chat-or-phone> --json
whatshell profile set-name "<name>" --json
whatshell profile set-about "<about>" --json
whatshell profile set-picture ./avatar.jpg --json
whatshell profile remove-picture --json
whatshell blocking list --json
whatshell blocking block <contact> --json
whatshell blocking unblock <contact> --json
whatshell blocking is-blocked <contact> --json
whatshell status text --message "<text>" --to <recipient> --json
whatshell status image --file ./image.jpg --to <recipient> --caption "<caption>" --json
whatshell status video --file ./video.mp4 --to <recipient> --caption "<caption>" --json
whatshell status revoke --id <status-message-id> --to <recipient> --json
```

Treat these as side-effecting user-account actions.

## Export

```bash
whatshell export messages --chat <chat-or-phone> --format jsonl --output ./chat.jsonl
whatshell export analytics --json
```

Use exports when the user asks for summaries, audits, or offline processing of larger chat histories.
