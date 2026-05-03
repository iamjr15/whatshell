# wacli

`wacli` is a Rust WhatsApp Web CLI designed for coding agents and terminal workflows. It uses the personal WhatsApp Web multi-device protocol through `whatsapp-rust`, stores a local SQLite message index, and keeps command output predictable for automation.

This is not the WhatsApp Business Cloud API. It links like WhatsApp Web and depends on WhatsApp's unofficial Web protocol behavior.

## Install

From this directory:

```bash
npm install -g .
```

The npm `postinstall` script builds the Rust release binary with Cargo and installs a small Node launcher. You can also use Cargo directly:

```bash
cargo install --path .
```

The npm install also installs the WACLI Agent Skill globally for coding agents:

- Claude Code: `~/.claude/skills/wacli`
- Codex and OpenCode agent-compatible discovery: `~/.agents/skills/wacli`

OpenCode also discovers the Claude-compatible and agent-compatible locations and de-duplicates by skill name, so WACLI does not install a third OpenCode-native copy by default. Set `WACLI_INSTALL_OPENCODE_SKILL=1` if you explicitly want `~/.config/opencode/skills/wacli` too. Set `WACLI_INSTALL_CODEX_HOME_SKILL=1` if you explicitly want a legacy `$CODEX_HOME/skills/wacli` copy. Set `WACLI_SKIP_SKILL_INSTALL=1` to skip skill installation.

## Authenticate

```bash
wacli auth
```

Scan the QR code in WhatsApp: `Settings > Linked Devices > Link a Device`.

Phone-number pairing is supported when WhatsApp allows it:

```bash
wacli auth --phone +15551234567
```

Status and local logout:

```bash
wacli auth status --json
wacli auth logout
```

`auth logout` removes the local session files. If you need to revoke the linked device remotely, remove it from WhatsApp's Linked Devices screen.

## Operational Model

WACLI keeps a local SQLite index built from WhatsApp Web sync data. Read commands are fastest and most useful after a sync:

```bash
wacli sync --once --json
wacli contacts sync --json
```

Use `--json` for automation and preserve returned message IDs, chat JIDs, group JIDs, status IDs, and invite links when you may need to reply, revoke, clean up, or verify an action later.

Live WhatsApp commands use a per-store process lock. Run live commands serially against the same store; if you need an isolated account or test profile, use a separate store:

```bash
wacli --store ~/.wacli-work auth
wacli --store ~/.wacli-work sync --once --json
```

Use `--read-only` when you want WACLI to reject side-effecting commands.

## WACLI Commands

Sync and listen:

```bash
wacli sync --once --json
wacli sync --follow --stream-jsonl
wacli listen --stream-jsonl
```

Send messages:

```bash
wacli send text --to +15551234567 --message "Build finished" --json
wacli send file --to 120363000000000000@g.us --file ./report.pdf --caption "Latest report"
wacli send react --to +15551234567 --id MESSAGE_ID --reaction "+1"
wacli send poll --to +15551234567 --question "Ship?" --option Yes --option No --json
wacli send location --to +15551234567 --latitude 12.9716 --longitude 77.5946 --name Bengaluru --json
wacli send contact --to +15551234567 --name "Ops Phone" --phone +15551234567 --json
```

Query the local index:

```bash
wacli chats list --json
wacli messages list --chat +15551234567 --limit 20 --json
wacli messages search "deployment failed" --limit 10 --json
wacli messages context --chat +15551234567 --id MESSAGE_ID --before 5 --after 5 --json
wacli messages reply --chat +15551234567 --id MESSAGE_ID --message "Looking now" --json
wacli contacts sync --json
wacli contacts list --json
wacli contacts search "Devansh" --json
wacli contacts check +15551234567 --json
wacli contacts info +15551234567 --json
wacli groups list --json
wacli presence typing --chat +15551234567 --json
wacli chats mark-read --chat +15551234567 --json
wacli media list --chat +15551234567 --json
wacli export messages --chat +15551234567 --format jsonl --output ./chat.jsonl
wacli export analytics --json
```

Message actions:

```bash
wacli messages edit --chat +15551234567 --id MESSAGE_ID --message "Edited text" --json
wacli messages revoke --chat +15551234567 --id MESSAGE_ID --json
wacli messages delete-for-me --chat +15551234567 --id MESSAGE_ID --json
wacli messages star --chat +15551234567 --id MESSAGE_ID --json
wacli media download --chat +15551234567 --id MESSAGE_ID --output ./downloads
```

Groups, profile, and status:

```bash
wacli groups create --subject "Build Room" --participant +15551234567 --dry-run --json
wacli groups add --jid 120363000000000000@g.us +15551234567 --json
wacli groups set-subject --jid 120363000000000000@g.us --subject "Build Room 2" --json
wacli groups set-description --jid 120363000000000000@g.us --description "Release coordination" --json
wacli groups set-description --jid 120363000000000000@g.us --clear --json
wacli groups lock --jid 120363000000000000@g.us --json
wacli groups ephemeral --jid 120363000000000000@g.us --seconds 86400 --json
wacli profile set-about "Available through wacli" --json
wacli blocking is-blocked +15551234567 --json
wacli status text --message "Working" --to +15551234567 --json
wacli status revoke --id STATUS_MESSAGE_ID --to +15551234567 --json
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
wacli doctor --json
wacli doctor --connect
```

## Storage

By default, `wacli` stores session and index data in the platform data directory. Override it per command or with `WACLI_STORE`:

```bash
wacli --store ~/.wacli-work auth
WACLI_STORE=~/.wacli-work wacli auth status --json
```

Files:

- `session.db`: WhatsApp Web session managed by `whatsapp-rust`
- `index.db`: local chat/message index with SQLite FTS5 when available
- `LOCK`: process lock for commands that touch the live WhatsApp session
- `media/`: local media downloads

## Research Basis

The command model borrows from the strongest public WhatsApp CLI and agent-oriented projects:

- `steipete/wacli`: SQLite-backed CLI, FTS search, JSON output, store locking, agent-focused command design
- `vicentereig/whatsapp-cli`: personal WhatsApp Web CLI patterns for Claude/Codex workflows
- `tulir/whatsmeow`: mature Go WhatsApp Web implementation and operational patterns
- `WhiskeySockets/Baileys`: TypeScript WhatsApp Web protocol coverage
- `jlucaso1/whatsapp-rust`: Rust WhatsApp Web client used as the runtime protocol implementation

## Notes

WhatsApp Web automation can break when WhatsApp changes its private protocol. Keep the linked device limited to accounts where this automation is acceptable, and avoid sending spam or high-volume automated traffic.
