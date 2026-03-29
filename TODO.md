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

10. **GitHub** (`interconnect-connector-github`) ✓ done — Issues as rooms.
    REST API polling (30s). Intents: AddComment, React, CloseIssue.
    `connect(token, owner, repo, issue_number)`. Prior art: utteranc.es.

11. **WhatsApp** (`interconnect-connector-whatsapp`) ✓ done — Business Cloud
    API via Graph API. Send works; recv requires webhook (returns None until
    implemented). `connect(phone_number_id, access_token, recipient)`.

12. **iMessage** (`interconnect-connector-imessage`) — requires Mac relay
    (BlueBubbles or similar). Reference: matrix-imessage. High setup friction;
    low priority unless relay story improves.

13. **Signal** (`interconnect-connector-signal`) ✓ done — `signal-cli`
    subprocess, JSON-RPC over stdio. E2EE transparent.
    `connect(signal_cli_path, account, recipient)`.

Skip for now:
- Raw email/IMAP — messy semantics, Zulip + mailing list cover the real needs
- Notion — cloud-owned, off-brand
- RSS — read-only (no intents); trivial but low value

### SQLite connector (`interconnect-connector-sqlite`) ✓ done (generic mode)

Two modes:

**Generic mode** (done) — `connect(path, table)`. Any schema, poll for changes
via `COUNT(*)/MAX(rowid)`. Intents: Execute/Insert/Delete.

**Chat log mode** (TODO) — `connect_chat(path, config)`. Owned schema, manages
a `messages` table. Each connector maps into it via a user-defined column
mapping in `interconnect.toml`:

```toml
[chat_log]
path = ".interconnect/chat.db"

[chat_log.columns]
id        = { path = "message.id",          type = "text",    primary_key = true }
author    = { path = "message.author.name", type = "text" }
timestamp = { path = "message.timestamp",   type = "integer" }
guild     = { path = "message.server.id",   type = "text",    nullable = true }
raw       = { path = "*",                   type = "json" }
```

- `path` is a dot-path into the snapshot JSON; `"*"` dumps the full platform blob
- `type`: `text`, `integer`, `real`, `boolean`, `json`, `blob`
- Missing paths → NULL (requires `nullable = true`)
- A default `--preset chat` template ships for common chat logging

**Schema design notes:**
- No single canonical `interconnect-schema` — schema is consumer-defined, not
  protocol-defined. The protocol is deliberately schema-agnostic.
- The column mapping DSL should be backend-agnostic. Future connectors
  (`interconnect-connector-postgres`, `interconnect-connector-duckdb`) reuse
  the same `interconnect.toml` column config.
- Platform-specific metadata (Discord embeds, Slack blocks, Matrix event
  content) goes in the `raw` JSON column — nothing lost, common fields
  queryable cross-platform.

### Daemon + Claude Code integration (`interconnect-daemon`) ✓ done (infrastructure)

A persistent daemon that owns long-lived room connections and exposes a
blocking CLI. Enables `claude -p` to participate in rooms interactively.

**CLI interface:**
```
interconnect recv <room>           # block until next unread message
interconnect recv --nowait <room>  # return pending messages or nothing
interconnect send <room> <intent>  # send intent to room
interconnect state <room>          # dump current snapshot
```

**Daemon responsibilities:**
- Holds live connections to all configured rooms
- Tracks a read cursor per room per session (delivers deltas, not full snapshots)
- Surfaces a Unix socket API that the CLI wraps

**Claude Code hook wiring:**
- `PreToolUse` / between-turns: `interconnect recv --nowait <room>` — inject
  any pending messages into context before Claude continues
- `PostToolUse`: forward tool name + result to room as an event (running log,
  not just final summary)
- `Stop`: forward Claude's response to room as an intent

**Env var convention:** hook sets `INTERCONNECT_REPLY_TO=<room>` when waking
the assistant so the `Stop` hook knows where to route the reply.

**Result:** The assistant never thinks about routing. It responds; hooks handle
where input came from and where output goes. `recv --block` for deliberate
waiting; `recv --nowait` in hooks for ambient message injection.

**Tool-agnostic by design.** The daemon knows nothing about Claude, Gemini,
Cursor, etc. Hook wiring is declared in `interconnect.toml`:

```toml
[[room]]
name = "work-chat"
connector = "slack"
channel = "C1234567890"

[[room]]
name = "session"
connector = "sqlite"
path = ".interconnect/session.db"

[[hook]]
event = "post_tool_use"
send_to = "session"

[[hook]]
event = "stop"
send_to = "work-chat"
reply_from = "work-chat"
```

`interconnect init --preset claude` reads `interconnect.toml` and emits the
corresponding `.claude/settings.json` hook entries. Presets are templates —
one per tool (Claude Code, Gemini CLI, Cursor, OpenCode, etc.). Community can
add presets without touching daemon code. The hook interface (`PostToolUse`,
`UserPromptSubmit`, `PreCompact`, `SessionStart`) is converging across all
major AI coding tools.

