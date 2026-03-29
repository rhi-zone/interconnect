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

### Multi-authority client (2026-03-29) ✓ done

`interconnect-client` crate: `Connection<T,I,S>`, `WsTransport`, `WsConnection`.
`Connection::established` needed for non-Interconnect handshakes (platform connectors).

### Platform connectors (2026-03-29)

Each platform is a room with its own authority. A connector is a `Transport`
implementation (plus `Connection::established` for non-native handshakes) that
presents the platform as an Interconnect room. In priority order:

1. **Discord** (`interconnect-connector-discord`) — immediate use case: steer
   an agent from Discord. Discord bot gateway → snapshots, intents → API calls.

2. **Filesystem** (`interconnect-connector-fs`) — local, owned. A watched
   directory as a room; file changes are snapshots, write intents modify files.
   Adapters: plaintext, markdown, serde (JSON/TOML/YAML).

3. **Zulip** (`interconnect-connector-zulip`) — self-hostable, open source,
   structured (stream + topic). "Your Zulip instance" is a room you own.

4. **Mailing list** (`interconnect-connector-maillist`) — one of the oldest
   owned rooms on the internet. Target: Listmonk API. A list is a room, a
   thread is part of the snapshot. Self-hosted = yours.

5. **Slack** (`interconnect-connector-slack`) — closed but ubiquitous.
   Same pattern as Discord, lower priority.

6. **Obsidian** (`interconnect-connector-obsidian`) — FS variant with vault
   semantics (backlinks, tags). Uses Obsidian's local REST plugin.

Skip for now:
- Raw email/IMAP — messy semantics, Zulip + mailing list cover the real needs
- Notion — cloud-hosted, you don't own it, off-brand

### Generalize docs language (2026-03-28)

Architecture, protocol, introduction docs still use game-heavy language (Tavern/Dungeon, physics, entities, player positions). Reframe:
- Game examples are fine as *examples*, but shouldn't be the primary framing
- Use generic terminology (room, authority, client) as the default
- Keep game examples alongside social, process, and agent examples

## Backlog

