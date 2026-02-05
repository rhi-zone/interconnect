# Why Interconnect?

What problems does this solve, and for whom?

## Why Don't People Do Federation?

Federation is rare outside a few protocols (email, Matrix, ActivityPub). Why?

### State consistency is a nightmare

Matrix and ActivityPub require merging state from multiple servers. This means:
- Understanding eventual consistency
- Implementing conflict resolution
- Dealing with split-brain scenarios
- Subtle bugs that only appear under network partitions

Most developers bounce off this complexity. It's not their core competency, and the failure modes are hard to reason about.

**Interconnect's answer:** Authoritative handoff. No merging. Each world/room/space has one authority. When you move, you switch authorities. Simple mental model.

### Trust is undefined

When you federate, you have to decide:
- Who can connect to you?
- What data do you accept from them?
- How do you handle spam and abuse from other servers?
- Who's responsible when things go wrong?

Most protocols leave this vague, leading to blocklists, allowlists, and ad-hoc policies.

**Interconnect's answer:** Explicit import policies. Each server declares what it accepts. Contraband is rejected at the border, not silently dropped or merged.

### Identity is a whole thing

Every federated protocol invents identity:
- Email: `user@domain`
- Matrix: `@user:server`
- ActivityPub: WebFinger + HTTP signatures
- Others: DIDs, keys, OAuth bridges

Picking wrong is expensive. Migrating is painful.

**Interconnect's answer:** Algorithm-agnostic identity (`scheme:payload`). Use Ed25519 keys, or delegate to your existing auth, or use `local:` for dev. Migrate later without protocol changes.

### Testing requires multiple servers

To test federation, you need multiple servers running. Local development becomes:
- Spin up server A on port 8001
- Spin up server B on port 8002
- Configure them to know about each other
- Hope your machine can run both

**Interconnect's answer:** Single-process testing mode. Run multiple "servers" in one process with `local:` identity. No network setup for basic testing.

### Libraries are incomplete

ActivityPub libraries often implement 60% of the spec. You end up reading the RFC and patching gaps. Matrix SDKs are better but complex.

**Interconnect's answer:** Complete lifecycle handling. Transport, framing, serialization, connection state, substrate caching, transfer handshake - all handled. You implement your domain logic, not protocol plumbing.

### Business model conflict

Federation works against lock-in. Most platforms profit from lock-in.

**Interconnect's answer:** None. This is a political/business problem, not a technical one. We can make federation easy; we can't make it profitable for incumbents.

---

## Why Is Netcode Hard?

Even without federation, multiplayer networking is notoriously difficult.

### Latency is physics

Speed of light is 3ms per 1000km. Cross-continental round trips are 50-150ms minimum. You can't fix this with code.

Mitigation requires:
- Client-side prediction
- Server reconciliation
- Interpolation and extrapolation
- Rollback for competitive games

**Interconnect's answer:** Partial. We provide the snapshot/intent model which supports these patterns, but implementing prediction/rollback is still your job. We handle the transport; you handle the game feel.

### Bandwidth adds up

Sending full world state every frame doesn't scale. You need:
- Delta compression (only send changes)
- Relevancy filtering (only send what this client can see)
- Priority systems (important updates first)

**Interconnect's answer:** Snapshot structure supports deltas. Relevancy and priority are application-level - you generate the snapshot, you decide what's in it.

### Clients lie

Any competitive game must assume clients are malicious:
- Aim assists, wall hacks, speed hacks
- Modified clients sending impossible inputs
- Packet manipulation

**Interconnect's answer:** Intent-based protocol. Clients send intent ("I want to move north"), servers compute results. Clients cannot declare state. This doesn't prevent all cheats (aimbots still work) but eliminates state injection.

### Connection chaos

Real networks have:
- Packet loss
- Out-of-order delivery
- Disconnects and reconnects
- Variable latency
- NAT traversal issues

**Interconnect's answer:** Built-in connection lifecycle with ghost mode. When authority is lost, you degrade gracefully instead of crashing. Reconnection logic is handled. (NAT traversal is still your problem or your transport's problem.)

### Everyone builds from scratch

There's no "Rails for multiplayer." Every studio implements their own netcode, often poorly, often from scratch.

**Interconnect's answer:** This is the goal. Bring your types (Intent, Snapshot, Passport), we handle the machinery. Whether we achieve "just use this" remains to be seen - the spike will tell us.

---

## What We Can't Solve

Being honest about limitations:

- **Latency** - Physics. Use prediction/rollback patterns.
- **NAT traversal** - Use WebRTC, TURN servers, or relay infrastructure.
- **Content moderation at scale** - Still a human/policy problem.
- **Adoption chicken-and-egg** - Federation only matters when others federate.
- **Business incentives** - Can't make lock-in unprofitable.

---

## Target Users

Who is Interconnect for?

1. **Indie game devs** who want multiplayer without implementing netcode from scratch
2. **Self-hosters** who want to run their own social/game servers and connect with friends
3. **Protocol designers** who want a federation primitive to build on
4. **Existing platforms** who want to add opt-in federation without rewriting everything

Who is it *not* for?

- AAA studios with dedicated netcode teams (they have their own solutions)
- Platforms that profit from lock-in (they won't adopt voluntarily)
- Applications requiring offline-first or true peer-to-peer (different architecture)
