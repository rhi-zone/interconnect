# Design Decisions

Architectural choices and their rationale.

## Reachability is Intentional

**Decision:** If the authoritative server is down, content is inaccessible.

**Rationale:** Matrix-style replication creates significant problems:
- State resolution attacks (craft conflicting events to burn CPU)
- History rewriting (inject fake events into the past)
- Split-brain scenarios (partition network, create conflicts, merge chaos)
- "Delete" doesn't mean delete (copies persist on other servers)

Matrix's Project Hydra (2025) addresses specific vulnerabilities—state resets from delayed federation traffic, room creation event hijacking—by replaying a broader event subgraph during state resolution and cryptographically binding room IDs to creation events. These are real improvements, but they make the state resolution algorithm *more* complex, not simpler. The fundamental attack surface—multiple untrusted servers contributing to shared mutable state—remains inherent to the model.

The alternative—single authority—is simpler and has clear semantics. When the authority is unreachable, you get ghost mode (substrate visible, simulation paused), not corrupted state.

**Tradeoff accepted:** Content availability depends on server availability. This is the same model as traditional websites.

## Replication is Opt-In

**Decision:** Servers can optionally replicate content, but it's not required or automatic.

**Patterns supported:**
- **Read replicas**: Authorized servers that can serve snapshots but not accept intents
- **Substrate caching**: Static content (geometry, media) cached at edges
- **Forwarding**: Server A forwards intents to Server B (like email relaying)
- **Mirrors**: Designated backup authorities (explicit, not emergent)

**Rationale:** Different applications have different needs:
- A game server might want zero replication (authoritative simulation)
- A social profile might want CDN caching (read scale)
- A community might want designated mirrors (resilience)

The protocol supports all of these. The application chooses.

## Algorithm-Agnostic Identity

**Decision:** Identity is a string of the form `algorithm:payload`. The protocol passes identity around; verification is deployment-specific.

**Supported schemes:**

| Scheme | Format | Trust model |
|--------|--------|-------------|
| `ed25519` | `ed25519:<fingerprint>` | Cryptographic (user holds key) |
| `dilithium` | `dilithium:<fingerprint>` | Post-quantum cryptographic |
| `url` | `url:alice@example.com` | Server vouches ("example.com says this is alice") |
| `local` | `local:player1` | Trust the connection (dev/LAN) |

**Rationale:**
- Start simple (`local:` for dev, `url:` for existing auth)
- Add cryptographic identity when needed
- Migrate to post-quantum later without protocol changes
- Different deployments have different trust requirements

**Cryptographic schemes** (ed25519, dilithium, etc.):
- User generates keypair, holds private key
- Identity is fingerprint of public key
- Passports signed with private key
- Any server can verify signature

**Delegated schemes** (url):
- Server at `example.com` authenticates users however it wants
- Other servers trust that server's attestation
- Simpler, but requires server trust

**Local schemes** (local):
- No verification, trust the transport
- Fine for single-machine dev, trusted LAN, or behind existing auth proxy

**Open questions:**
- Key rotation and recovery (for cryptographic schemes)
- Human-readable names (petnames? DNS? something else?)
- Key discovery (how do you find someone's public key?)

## Transport-Agnostic Core

**Decision:** The protocol defines message semantics. Transport bindings define delivery.

**Core primitives (transport-independent):**
- **Manifest**: What this server offers/requires
- **Intent**: Client requests action
- **Snapshot**: Server broadcasts current state
- **Transfer**: Handoff with signed passport

**Transport bindings:**

| Aspect | Stream (WebSocket) | Request (HTTP) |
|--------|-------------------|----------------|
| Connection | Persistent | Per-request |
| Snapshots | Continuous flow | On-demand fetch |
| Intents | Immediate send | POST request |
| Use case | Games, real-time | Social, async |

**Rationale:** One conceptual protocol, optimized delivery for different use cases. If the bindings diverge significantly, they become separate protocols—but we start unified and split only if necessary.

## Subscription is App-Level

**Decision:** The protocol provides primitives. Applications define what "follow" or "subscribe" means.

**Protocol provides:**
- Ability to send intents (including "subscribe to X")
- Ability to receive snapshots (including "here's an update")
- Ability to transfer identity between servers

**Application defines:**
- What subscription intents exist
- How updates are delivered (push, pull, webhook)
- What "unsubscribe" means
- Rate limits and access control

**Rationale:** A game's "I'm in this room" is different from social's "I follow this account" is different from a forum's "I'm watching this thread." The protocol shouldn't encode assumptions about subscription semantics.

**Example:** A microblogging app might implement:
```
Intent::Subscribe { target: PublicKey }
Intent::Unsubscribe { target: PublicKey }
```
And the server decides whether to push updates, require polling, or use webhooks.

## Authority Over Consensus

**Decision:** Each piece of content has exactly one authoritative server at any time.

**Not supported:**
- Multiple servers merging state
- Conflict resolution algorithms
- Eventually consistent semantics

**Supported:**
- Authority transfer (Server A hands off to Server B permanently)
- Delegated authority (Server A temporarily delegates to Server B)
- Read replication (copies can serve reads, not writes)

**Rationale:** Consensus is expensive, complex, and has known attack surfaces. Single authority is simple: ask the authority, get the answer. If you can't reach the authority, you wait or see stale data—not corrupted data.

## Minimal Protocol, App-Defined Semantics

**Decision:** The protocol defines structure, not semantics. Application-specific data is opaque bytes.

**What the protocol defines:**
- Message framing (how to delimit messages on the wire)
- Identity format (`algorithm:payload`)
- Substrate verification (content-addressed by hash)

**What the protocol leaves to applications:**
- Intent structure (your app defines what actions exist)
- Snapshot structure (your app defines what state looks like)
- Passport structure (your app defines what transfers between servers)
- Server addresses (URL, IP, DID, whatever your app uses)
- Content references (hash, path, ID, whatever makes sense)

**Example:**

```rust
// Protocol level - generic
struct Transfer {
    destination: Bytes,       // Opaque to protocol
    passport: Bytes,          // Opaque to protocol
    identity: Identity,       // Protocol-defined format
    signature: Option<Bytes>, // Optional, scheme-dependent
}

// Application level - you define these
struct MyDestination { url: String, world_id: u64 }
struct MyPassport { username: String, inventory: Vec<Item>, stats: Stats }
```

**Rationale:** The protocol should fit into existing systems, not impose new addressing or data schemes. A game, a social network, and a forum have different needs - the protocol provides the handoff machinery, applications provide the meaning.

## Intent Over State

**Decision:** Clients send intent ("I want to do X"), servers compute results ("You are now Y").

**Not supported:**
- Clients declaring their own state
- Clients sending deltas to merge
- Trust in client-provided data

**Rationale:** Clients can lie, be buggy, or be malicious. The server is the source of truth. Clients express what they want; servers decide what happens.

**Example:**
```
// Client sends
Intent::Move { direction: North }

// Server responds (in next snapshot)
Snapshot { player_position: (5, 3), ... }

// NOT: Client sends "I am now at (5, 3)"
```
