# Use Cases

Interconnect isn't game-specific. The protocol primitives work anywhere you want federation without consensus overhead.

## Design Goal: Invisible Integration

Adding Interconnect to an existing system should be opt-in and non-invasive:

- **Standalone works fine.** A server that never calls the transfer API is just a normal server.
- **Federation is additive.** You add transfer endpoints; existing logic stays unchanged.
- **No rewrite required.** If adoption requires rebuilding from scratch, nobody will use it.

The protocol defines how servers hand off users and what data travels with them. What happens inside each server is not Interconnect's concern.

---

## Virtual Worlds

### Sharded Open World

One organization runs multiple servers for different geographic regions. Players crossing a boundary get handed off seamlessly.

- Same trust level across all servers
- Full inventory/stats transfer
- Appears as one continuous world to players

### Guest Worlds

A creator builds a dungeon on their personal server and advertises a portal. Players from the "main" server can visit, complete the dungeon, then return home.

- Different trust levels
- Main server defines what loot players can bring back
- Creator server defines what gear visitors can use

### Permadeath Roguelike Network

A network of hardcore servers that don't trust each other's progression. On transfer, you keep your name and cosmetics; everything else resets.

- Extreme import policy
- Prevents stat inflation across the network
- Each server is a fresh challenge

### Shared Social Hub

Multiple game servers connect to a neutral social space. The hub only accepts avatar appearance—no weapons, no stats, no gameplay items.

- Hub is a "demilitarized zone"
- Players from competing games can interact
- No cross-contamination of game balance

---

## Social Platforms

The same primitives apply. Your server is authoritative for your content. Others visit or subscribe; they don't get a replica to modify.

### Federated Microblogging

Like Twitter, but you own your posts.

- Your server hosts your timeline
- Followers' servers fetch posts from you (or subscribe to a stream)
- No "other instance has a stale copy" problem
- Deletion is real deletion—you control the authoritative copy

**Import policy example:** A server might refuse to display posts from accounts less than 30 days old (spam filtering).

### Federated Communities

Like Discord servers or forums, but portable identity.

- Each community is authoritative for its channels/threads
- Your identity (name, avatar, reputation) transfers when you join
- Community defines what reputation it accepts (or ignores)

**Import policy example:** A programming forum might import your GitHub contribution count but ignore your karma from meme communities.

### Federated Photo Sharing

Like Instagram, but your media stays yours.

- Photos live on your server
- Followers see them via your server or authorized CDN
- You can actually delete things
- No platform owns your content

**Import policy example:** A photography community might only federate with servers that enforce minimum image quality or metadata standards.

### Federated Profiles

Like Facebook, but you control your social graph.

- Your profile, friends list, and posts live on your server
- You grant access to specific servers/users
- Moving to a new server means updating DNS, not begging for data export

**Import policy example:** A professional network might import employment history but strip out personal posts.

---

## How This Differs from ActivityPub

ActivityPub (Mastodon, etc.) uses state replication:

1. You post on Server A
2. Server B gets a copy
3. Server C gets a copy
4. Now there are three "truths"
5. Deletion becomes "please delete your copy"

Interconnect uses authoritative handoff:

1. You post on Server A
2. Server B's users *visit* Server A to see it (or subscribe to a stream)
3. One truth, one authority
4. Deletion is deletion

The tradeoff: ActivityPub is resilient to server death (copies survive). Interconnect requires the authority to be reachable—but the substrate layer provides graceful degradation (static content survives, dynamic state pauses).

---

## Hybrid Patterns

### Caching Without Authority

A server can cache substrate (static content) from another server without claiming authority. If the origin dies, the cached substrate remains viewable in "ghost mode"—read-only, no interactions.

### Authority Transfer

A server can permanently transfer authority for content to another server. This is a migration, not federation—the old server stops being authoritative.

### Delegated Authority

A server can temporarily delegate authority to another server (e.g., for load balancing or maintenance). The original server remains the source of truth but isn't actively serving.

---

## Anti-Use-Cases

Interconnect is **not** a good fit for:

- **Collaborative editing** where multiple users modify the same document simultaneously (use CRDTs or OT)
- **Consensus systems** where all participants must agree on truth (use blockchain or Raft)
- **Offline-first apps** where devices need to work without connectivity and sync later (use local-first sync)

The protocol assumes one authority is reachable and makes decisions. When it's not, you get ghost mode, not continued operation.
