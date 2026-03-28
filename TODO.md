# Interconnect TODO

## Priority

### Generalize protocol types (2026-03-28)

The core protocol types are game-flavored. They need to be domain-agnostic:
- `Intent` enum has `Move { direction }`, `Interact { target: EntityId }`, `UseItem` — should be `Intent<T>` where the room defines T
- `Snapshot` has `entities: Vec<EntityState>` — should be `Snapshot<T>` where the authority defines what state looks like
- `Manifest` has `physics_config: PhysicsConfig`, `allowed_items` — should carry room-defined capabilities/requirements
- Keep game-specific types as one example implementation, not the protocol definition

### Transport trait (2026-03-28)

Protocol currently assumes WebSocket. Add a `Transport` trait:
- Protocol layer speaks messages, transport layer moves bytes
- Implementations: WebSocket, Unix socket, HTTP long-poll, message queue
- A Discord bot adapter is a transport — the protocol doesn't know or care

### Process-as-room spike — agent steering via Discord (2026-03-28)

**This is the immediate use case.** Minimal end-to-end: a running process is a room, reachable from Discord.
- Authority: wraps a process (initially Claude Code via hooks), accepts intents, emits snapshots
- Transport: Discord bot adapter — intents arrive as Discord messages, snapshots go back as messages
- No federation needed for the spike — just one room, one client, one transport
- Proves the protocol works for something real, today
- Expand to multi-transport (terminal + Discord simultaneously) as second step

### Generalize docs language (2026-03-28)

Architecture, protocol, introduction docs still use game-heavy language (Tavern/Dungeon, physics, entities, player positions). Reframe:
- Game examples are fine as *examples*, but shouldn't be the primary framing
- Use generic terminology (room, authority, client) as the default
- Keep game examples alongside social, process, and agent examples

## Backlog

### Update CLAUDE.md — cargo test -q preference (2026-03-27)

When interconnect is clean, update CLAUDE.md Workflow section:
- Change `cargo test` to `cargo test -q` in the example command
- Add note: "Prefer `cargo test -q` over `cargo test` — quiet mode only prints failures, significantly reducing output noise and context usage."
Conventional commit: `docs: prefer cargo test -q to reduce output noise`
