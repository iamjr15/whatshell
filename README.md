# whatshell

`whatshell` is a Rust WhatsApp Web CLI designed for coding agents and terminal workflows. It uses the personal WhatsApp Web multi-device protocol through `whatsapp-rust`, stores a local SQLite message index, and keeps command output predictable for automation.

This is not the WhatsApp Business Cloud API. It links like WhatsApp Web and depends on WhatsApp's unofficial Web protocol behavior.

## Install

From npm:

```bash
npm install -g whatshell
```

From this directory:

```bash
npm install -g .
```

The npm `postinstall` script builds the Rust release binary with Cargo and installs a small Node launcher. You can also use Cargo directly:

```bash
cargo install --path .
```

The npm install also installs the Whatshell Agent Skill globally for coding agents:

- Claude Code: `~/.claude/skills/whatshell`
- Codex and OpenCode agent-compatible discovery: `~/.agents/skills/whatshell`

OpenCode also discovers the Claude-compatible and agent-compatible locations and de-duplicates by skill name, so Whatshell does not install a third OpenCode-native copy by default. Set `WHATSHELL_INSTALL_OPENCODE_SKILL=1` if you explicitly want `~/.config/opencode/skills/whatshell` too. Set `WHATSHELL_INSTALL_CODEX_HOME_SKILL=1` if you explicitly want a legacy `$CODEX_HOME/skills/whatshell` copy. Set `WHATSHELL_SKIP_SKILL_INSTALL=1` to skip skill installation.

## Authenticate

```bash
whatshell auth
```

Scan the QR code in WhatsApp: `Settings > Linked Devices > Link a Device`.

Phone-number pairing is supported when WhatsApp allows it:

```bash
whatshell auth --phone +15551234567
```

Status and local logout:

```bash
whatshell auth status --json
whatshell auth logout
```

`auth logout` removes the local session files. If you need to revoke the linked device remotely, remove it from WhatsApp's Linked Devices screen.

## Operational Model

Whatshell keeps a local SQLite index built from WhatsApp Web sync data. Read commands are fastest and most useful after a sync:

```bash
whatshell sync --once --json
whatshell contacts sync --json
```

Use `--json` for automation and preserve returned message IDs, chat JIDs, group JIDs, status IDs, and invite links when you may need to reply, revoke, clean up, or verify an action later.

Live WhatsApp commands use a per-store process lock. Run live commands serially against the same store; if you need an isolated account or test profile, use a separate store:

```bash
whatshell --store ~/.whatshell-work auth
whatshell --store ~/.whatshell-work sync --once --json
```

Use `--read-only` when you want Whatshell to reject side-effecting commands.

## Whatshell Commands

Sync and listen:

```bash
whatshell sync --once --json
whatshell sync --follow --stream-jsonl
whatshell listen --stream-jsonl
```

Send messages:

```bash
whatshell send text --to +15551234567 --message "Build finished" --json
whatshell send file --to 120363000000000000@g.us --file ./report.pdf --caption "Latest report"
whatshell send react --to +15551234567 --id MESSAGE_ID --reaction "+1"
whatshell send poll --to +15551234567 --question "Ship?" --option Yes --option No --json
whatshell send location --to +15551234567 --latitude 12.9716 --longitude 77.5946 --name Bengaluru --json
whatshell send contact --to +15551234567 --name "Ops Phone" --phone +15551234567 --json
```

Query the local index:

```bash
whatshell chats list --json
whatshell messages list --chat +15551234567 --limit 20 --json
whatshell messages search "deployment failed" --limit 10 --json
whatshell messages context --chat +15551234567 --id MESSAGE_ID --before 5 --after 5 --json
whatshell messages reply --chat +15551234567 --id MESSAGE_ID --message "Looking now" --json
whatshell contacts sync --json
whatshell contacts list --json
whatshell contacts search "Devansh" --json
whatshell contacts check +15551234567 --json
whatshell contacts info +15551234567 --json
whatshell groups list --json
whatshell presence typing --chat +15551234567 --json
whatshell chats mark-read --chat +15551234567 --json
whatshell media list --chat +15551234567 --json
whatshell export messages --chat +15551234567 --format jsonl --output ./chat.jsonl
whatshell export analytics --json
```

Message actions:

```bash
whatshell messages edit --chat +15551234567 --id MESSAGE_ID --message "Edited text" --json
whatshell messages revoke --chat +15551234567 --id MESSAGE_ID --json
whatshell messages delete-for-me --chat +15551234567 --id MESSAGE_ID --json
whatshell messages star --chat +15551234567 --id MESSAGE_ID --json
whatshell media download --chat +15551234567 --id MESSAGE_ID --output ./downloads
```

Groups, profile, and status:

```bash
whatshell groups create --subject "Build Room" --participant +15551234567 --dry-run --json
whatshell groups add --jid 120363000000000000@g.us +15551234567 --json
whatshell groups set-subject --jid 120363000000000000@g.us --subject "Build Room 2" --json
whatshell groups set-description --jid 120363000000000000@g.us --description "Release coordination" --json
whatshell groups set-description --jid 120363000000000000@g.us --clear --json
whatshell groups lock --jid 120363000000000000@g.us --json
whatshell groups ephemeral --jid 120363000000000000@g.us --seconds 86400 --json
whatshell profile set-about "Available through whatshell" --json
whatshell blocking is-blocked +15551234567 --json
whatshell status text --message "Working" --to +15551234567 --json
whatshell status revoke --id STATUS_MESSAGE_ID --to +15551234567 --json
```

## Current WhatsApp Web Surface

Implemented now:

- Auth: QR and phone-number pairing, status, local logout.
- Read/index: history sync, live listen, all-message listing, chat filters, FTS search, show, context, JSON/JSONL/CSV export, analytics.
- Reply/send: text, quoted replies, file/media upload, reactions, polls, locations, contact cards.
- Message actions: edit own messages, revoke, delete-for-me, star/unstar.
- Media: list indexed media and download/decrypt stored media messages.
- Contacts: indexed contact/chats list, live number registration check, live contact info lookup.
- Chats: list, archive/unarchive, pin/unpin, mute/unmute, mark read/unread, delete chat.
- Groups: list joined groups, group info/invite info, invite link/reset, leave, create, rename, description, participant add/remove/promote/demote, approval requests, lock/unlock, announcement mode, disappearing messages, member-add mode.
- Presence: online/offline, typing, recording, paused indicators.
- Profile: set push name, set about text, set/remove own profile picture.
- Blocking: list, block, unblock, check blocked state.
- Status/stories: text, image, video, revoke with explicit recipient lists.
- Packaging: Rust binary with npm launcher/install script.

Remaining parity boundaries:

- MCP server mode for direct Claude/Codex tool registration.
- Newsletters/channels and communities are intentionally not exposed until the underlying protocol calls can be made reliable.
- Joining groups by invite is intentionally not exposed until it is proven in a reversible live test.
- Full voice/video call audio participation is not realistically supported by current public WhatsApp Web protocol libraries.
- Link previews and stickers are best-effort through normal media/text sends; WhatsApp may derive previews client-side or server-side depending on account and protocol behavior.

Diagnostics:

```bash
whatshell doctor --json
whatshell doctor --connect
```

## Storage

By default, `whatshell` stores session and index data in the platform data directory. Override it per command or with `WHATSHELL_STORE`:

```bash
whatshell --store ~/.whatshell-work auth
WHATSHELL_STORE=~/.whatshell-work whatshell auth status --json
```

Files:

- `session.db`: WhatsApp Web session managed by `whatsapp-rust`
- `index.db`: local chat/message index with SQLite FTS5 when available
- `LOCK`: process lock for commands that touch the live WhatsApp session
- `media/`: local media downloads

## Research Basis

The command model borrows from the strongest public WhatsApp CLI and agent-oriented projects:

- `vicentereig/whatsapp-cli`: personal WhatsApp Web CLI patterns for Claude/Codex workflows
- `tulir/whatsmeow`: mature Go WhatsApp Web implementation and operational patterns
- `WhiskeySockets/Baileys`: TypeScript WhatsApp Web protocol coverage
- `jlucaso1/whatsapp-rust`: Rust WhatsApp Web client used as the runtime protocol implementation

## Notes

WhatsApp Web automation can break when WhatsApp changes its private protocol. Keep the linked device limited to accounts where this automation is acceptable, and avoid sending spam or high-volume automated traffic.
