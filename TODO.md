# Interconnect TODO

## Priority

### Generalize protocol types (2026-03-28)

The core protocol types are game-flavored. They need to be domain-agnostic:
- `Intent` enum has `Move { direction }`, `Interact { target: EntityId }`, `UseItem` — should be `Intent<T>` where the room defines T
- `Snapshot` has `entities: Vec<EntityState>` — should be `Snapshot<T>` where the authority defines what state looks like
- `Manifest` has `physics_config: PhysicsConfig`, `allowed_items` — should carry room-defined capabilities/requirements
- Keep game-specific types as one example implementation, not the protocol definition

### Transport trait (2026-03-29)

Protocol currently assumes WebSocket. Add a `Transport` trait:
- Protocol layer speaks messages, transport layer moves bytes
- Implementations: WebSocket, Unix socket, HTTP long-poll, message queue
- Note: Discord is NOT a transport — it's a separate authority. Transports are how a client reaches an authority, not how authorities relate to each other.

### Process-as-room spike — agent steering (2026-03-29)

**This is the immediate use case.** Minimal end-to-end: a running process is a room.
- The agent is an authority (owns the running session)
- Discord is a separate authority (owns its channels)
- You're a client connected to both simultaneously
- The spike proves: a process can be an interconnect authority, and a client can be in multiple rooms at once
- Start with Claude Code hooks as the agent-side integration

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
