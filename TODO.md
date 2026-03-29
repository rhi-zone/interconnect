# Interconnect TODO

## Priority

### Generalize protocol types (2026-03-28) ✓ done

Core was already generic: `ClientWire<I>`, `ServerWire<S>`, `Authority` with
associated types, `Manifest.metadata: serde_json::Value`. Game-specific types
exist only in `examples/game/`. Nothing to change.

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

1. **Discord** (`interconnect-connector-discord`) ✓ done — gateway events →
   snapshots, intents → HTTP API calls. `connect(token, channel_id)` returns
   a `DiscordConnection` usable in `tokio::select!` alongside any other room.

2. **Filesystem** (`interconnect-connector-fs`) ✓ done — inotify watcher,
   text files as snapshots, WriteFile/DeleteFile intents.

3. **Zulip** (`interconnect-connector-zulip`) ✓ done — HTTP long-poll event
   queue, stream+topic filtering, rustls.

4. **Mailing list** (`interconnect-connector-maillist`) ✓ done — Listmonk
   REST API, 30s polling, campaign send intent. 6 tests.

5. **Slack** (`interconnect-connector-slack`) ✓ done — Socket Mode WebSocket,
   user display name resolution, ack handling, rustls.

6. **Obsidian** (`interconnect-connector-obsidian`) — FS variant with vault
   semantics (backlinks, tags). Uses Obsidian's local REST plugin. Low priority.

Skip for now:
- Raw email/IMAP — messy semantics, Zulip + mailing list cover the real needs
- Notion — cloud-hosted, you don't own it, off-brand

### Generalize docs language (2026-03-28) ✓ done

Architecture, protocol, introduction docs reframed. Game examples kept as one
example among many (social, process, agent). Generic terminology (room,
authority, client) is now the default.

## Backlog

### More platform connectors

Use Matrix bridge implementations as reference for platform quirks.

7. **Telegram** (`interconnect-connector-telegram`) ✓ done — Bot API via
   reqwest. Long-poll `getUpdates`, `sendMessage`. `connect(bot_token, chat_id)`.

8. **Matrix** (`interconnect-connector-matrix`) ✓ done — Client-Server API via
   reqwest. Long-poll `/sync`, `PUT /send`. `connect(homeserver, access_token, room_id)`.

9. **IRC** (`interconnect-connector-irc`) ✓ done — plain TCP, RFC 1459.
   `connect(server, port, nick, channel)`. Auto-PONG.

10. **GitHub** (`interconnect-connector-github`) — Issues/PRs/Discussions as
    rooms. REST API + webhooks. Snapshot = comment thread. Intents: AddComment,
    React, Close. Prior art: utteranc.es. Reference: matrix-github.

11. **WhatsApp** (`interconnect-connector-whatsapp`) — unofficial protocol via
    `whatsmeow` (Go) or similar Rust port. Reference: mautrix-whatsapp. Risk:
    account ban policy; implement with care.

12. **iMessage** (`interconnect-connector-imessage`) — requires Mac relay
    (BlueBubbles or similar). Reference: matrix-imessage. High setup friction;
    low priority unless relay story improves.

13. **Signal** (`interconnect-connector-signal`) — `signal-cli` subprocess or
    native Rust via `libsignal`. Reference: mautrix-signal. E2EE complicates
    snapshot model.

Skip for now:
- Raw email/IMAP — messy semantics, Zulip + mailing list cover the real needs
- Notion — cloud-owned, off-brand
- RSS — read-only (no intents); trivial but low value

