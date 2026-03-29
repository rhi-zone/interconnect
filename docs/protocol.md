# Protocol Reference

## Connection Lifecycle

```
CONNECTING → LOADING_SUBSTRATE → SYNCING → LIVE → GHOST
```

### States

| State | Description |
|-------|-------------|
| CONNECTING | Establishing WebSocket connection |
| LOADING_SUBSTRATE | Fetching/verifying static room data |
| SYNCING | Receiving initial snapshot |
| LIVE | Normal operation, sending intent, receiving snapshots |
| GHOST | Authority lost, substrate-only access |

## Message Formats

All messages are encoded as MessagePack.

### Client → Server

```rust
enum ClientMessage {
    Intent(Intent),
    AckSnapshot { tick: u64 },
    RequestTransfer { destination: RoomId },
}
```

### Server → Client

```rust
enum ServerMessage {
    Manifest(Manifest),
    Snapshot(Snapshot),
    Transfer(Transfer),
    Reject { reason: String },
}
```

## Intent Types

Intents are application-defined — the protocol carries them as opaque bytes. Common patterns:

```rust
enum Intent {
    // Actions — what the client wants to do
    Perform { action: ActionId, target: Option<TargetId> },

    // Communication
    Send { channel: Channel, message: String },
    Emote { emote: EmoteId },

    // Room modification
    Place { object: ObjectRef, location: Location },
    Modify { target: ObjectId, modification: Modification },
}
```

For a game, intents might include `Move { direction: Vec2 }` or `UseItem { slot: usize }`. For a social room, intents might include `Post { content: String }` or `React { target: PostId, reaction: ReactionId }`. For a process room, intents might include `Abort`, `Retry`, or `AdjustParameter { key: String, value: Value }`.

## Snapshot Structure

```rust
struct Snapshot {
    tick: u64,
    timestamp: Instant,

    // Delta from last acknowledged snapshot
    entries_added: Vec<StateEntry>,
    entries_removed: Vec<EntryId>,
    entries_changed: Vec<(EntryId, EntryDelta)>,

    // Events since last snapshot
    events: Vec<RoomEvent>,
}
```

## Transfer Protocol

When crossing room boundaries:

1. Client sends `RequestTransfer { destination }`
2. Authority validates (can client leave? does destination exist?)
3. Authority sends `Transfer { destination, passport, signature }`
4. Client disconnects from current authority
5. Client connects to destination with passport
6. Destination validates passport signature
7. Destination applies import policy
8. Client enters new room

## Availability States

```rust
enum Availability {
    Live,    // Connected to authority
    Cached,  // Authority lost, using local substrate
    Void,    // No authority, no cached substrate
}
```
