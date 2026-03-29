# Interconnect TODO

## Priority

### Generalize protocol types (2026-03-28)

The core protocol types are game-flavored. They need to be domain-agnostic:
- `Intent` enum has `Move { direction }`, `Interact { target: EntityId }`, `UseItem` — should be `Intent<T>` where the room defines T
- `Snapshot` has `entities: Vec<EntityState>` — should be `Snapshot<T>` where the authority defines what state looks like
- `Manifest` has `physics_config: PhysicsConfig`, `allowed_items` — should carry room-defined capabilities/requirements
- Keep game-specific types as one example implementation, not the protocol definition

### Transport trait (2026-03-29) ✓ done

`Transport` trait added to `interconnect-core`. Abstracts byte-moving.
WebSocket, Unix socket, etc. are implementations. Discord is NOT a transport.

### Process-as-room spike (2026-03-29) ✓ done

`examples/process`: wraps a subprocess as a room authority. Intent = stdin
input, Snapshot = stdout/stderr lines. WebSocket server. Proves the protocol
works outside of game/chat use cases.

### Multi-authority client (2026-03-29)

The model: a client connected to multiple authorities simultaneously.
- `interconnect-client` crate — manages one or more authority connections
- Relay logic: message in room A → intent to room B
- Discord authority: wraps the Discord API as a room (separate task)
- Proves: you can be in Discord and a process room at the same time

### Generalize docs language (2026-03-28)

Architecture, protocol, introduction docs still use game-heavy language (Tavern/Dungeon, physics, entities, player positions). Reframe:
- Game examples are fine as *examples*, but shouldn't be the primary framing
- Use generic terminology (room, authority, client) as the default
- Keep game examples alongside social, process, and agent examples

## Backlog

