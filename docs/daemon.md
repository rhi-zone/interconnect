# Daemon

The `interconnect-daemon` is a persistent process that owns long-lived room connections and exposes them to short-lived CLI invocations. Without a daemon, each CLI call would have to connect, authenticate, and tear down — expensive for platforms with slow handshakes (Slack Socket Mode, Discord gateway) and impossible for workflows that need to receive messages across multiple tool invocations.

The daemon runs in the background. The `interconnect` CLI talks to it over a Unix socket. CLI commands complete in milliseconds.

## Socket Location

Default: `~/.interconnect/daemon.sock`

Override with the `INTERCONNECT_SOCK` environment variable or the `--socket` flag:

```sh
interconnect --socket /run/my/daemon.sock recv work-chat
INTERCONNECT_SOCK=/run/my/daemon.sock interconnect list
```

## Configuration: `interconnect.toml`

The daemon reads `interconnect.toml` at startup. Each `[[room]]` entry declares one connection. The `name` field is how you refer to the room in all CLI commands. The `connector` field selects the backend. All other fields are connector-specific options.

### Slack

```toml
[[room]]
name       = "work-chat"
connector  = "slack"
bot_token  = "xoxb-..."
app_token  = "xapp-..."
channel_id = "C1234567890"
```

Requires a Slack app with Socket Mode enabled, the `channels:history` and `chat:write` bot scopes, and the `connections:write` app-level scope.

### SQLite (generic)

Generic mode connects to any existing SQLite table and polls for new rows.

```toml
[[room]]
name      = "events"
connector = "sqlite"
path      = ".interconnect/events.db"
table     = "log"
```

Intents: `Execute`, `Insert`, `Delete`.

### SQLite (chat log)

Chat log mode manages its own `messages` table. A column mapping in `interconnect.toml` describes how to extract fields from the platform's snapshot JSON.

```toml
[[room]]
name      = "archive"
connector = "sqlite"
path      = ".interconnect/chat.db"

[room.chat_log]

[room.chat_log.columns]
id        = { path = "message.id",          type = "text",    primary_key = true }
author    = { path = "message.author.name", type = "text" }
timestamp = { path = "message.timestamp",   type = "integer" }
guild     = { path = "message.server.id",   type = "text",    nullable = true }
raw       = { path = "*",                   type = "json" }
```

`path` is a dot-path into the snapshot JSON. `"*"` stores the full platform blob. Missing paths produce NULL and require `nullable = true`.

### Discord

```toml
[[room]]
name       = "announcements"
connector  = "discord"
token      = "Bot ..."
channel_id = 1234567890123456789
```

### IRC

```toml
[[room]]
name    = "freenode-rust"
connector = "irc"
server  = "irc.libera.chat"
port    = 6667
nick    = "mybot"
channel = "#rust"
```

### Telegram

```toml
[[room]]
name      = "alerts"
connector = "telegram"
bot_token = "123456:ABC-..."
chat_id   = -1001234567890
```

### Matrix

```toml
[[room]]
name         = "team"
connector    = "matrix"
homeserver   = "https://matrix.example.com"
access_token = "syt_..."
room_id      = "!abc123:example.com"
```

### Other supported connectors

| Connector    | Key options |
|-------------|-------------|
| `zulip`     | `realm`, `email`, `api_key`, `stream`, `topic` |
| `maillist`  | `base_url`, `username`, `password`, `list_id` |
| `signal`    | `signal_cli_path`, `account`, `recipient` |
| `github`    | `token`, `owner`, `repo`, `issue_number` |
| `whatsapp`  | `phone_number_id`, `access_token`, `recipient_phone` |
| `fs`        | `root` |

## CLI Commands

All commands contact the running daemon over the socket. They fail immediately if the daemon is not running.

### `recv`

Block until at least one new message arrives in `<room>`, then print it as JSON and exit.

```sh
interconnect recv work-chat
```

The daemon tracks a read cursor per room. `recv` returns only messages received since the last `recv` call — it delivers deltas, not the full snapshot.

### `recv --nowait`

Return any pending unread messages immediately. If there are none, print nothing and exit.

```sh
interconnect recv --nowait work-chat
```

This is the non-blocking variant. It is used in hooks that run on every tool call — inject pending messages if any exist, otherwise do nothing and let the assistant continue.

### `send`

Send a JSON intent payload to a room.

```sh
interconnect send work-chat '{"type":"message","text":"build passed"}'
```

The daemon forwards the intent to the connector. What happens next depends on the connector — for Slack this posts a message, for SQLite this inserts a row.

### `state`

Print the current snapshot for a room.

```sh
interconnect state work-chat
```

Returns the most recent state the daemon received from the connector, as JSON.

### `list`

List all rooms configured in `interconnect.toml`.

```sh
interconnect list
```

### `watch`

Block on a room and invoke a shell command whenever a message arrives. The message JSON is piped to the command's stdin. `INTERCONNECT_REPLY_TO` is set to the room name for the duration of the command.

```sh
interconnect watch work-chat --exec 'claude -p "$(cat)"'
```

`watch` loops indefinitely. Each message triggers one invocation of the command. The previous invocation must complete before the next message is processed.

This is how you build a reactive agent: messages in a room wake the assistant, which can reply back to the same room via `INTERCONNECT_REPLY_TO`.

### `init --preset`

Generate integration hooks for a named preset and write them to a file.

```sh
# Print to stdout
interconnect init --preset claude

# Write directly to .claude/settings.json
interconnect init --preset claude --output .claude/settings.json
```

By default, reads `interconnect.toml` from the current directory. Override with `--config`:

```sh
interconnect init --preset claude --config ~/myproject/interconnect.toml --output .claude/settings.json
```

Currently available preset: `claude`. See [Agent Orchestration](/agent-orchestration) for what the generated hooks do.

## `INTERCONNECT_REPLY_TO`

When this environment variable is set, it names the room that hooks should route output to. The variable is not used by the daemon itself — it is a convention for hook scripts and wrapper invocations.

The `watch` command sets it automatically. When invoking an assistant manually, set it yourself:

```sh
INTERCONNECT_REPLY_TO=work-chat claude -p "summarize the last hour"
```

The `Stop` hook inside `.claude/settings.json` reads this variable and routes Claude's final response to that room. If the variable is not set, the hook does nothing (errors are suppressed).
