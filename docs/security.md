# Security Model

## Eliminated Attack Classes

By using authoritative handoff instead of state resolution:

### History Rewrite Attack

**Matrix Risk**: Inject fake events into the past to change current state.

**Interconnect Defense**: Impossible. Clients cannot send state, only intent. Server computes all results.

### Split-Brain Attack

**Matrix Risk**: Partition network, create two realities, merge chaos.

**Interconnect Defense**: If Server A goes offline, the world pauses. You can't fork the world because Server A is the only machine with the valid simulation.

### State Bloom Attack

**Matrix Risk**: Flood room with metadata updates, replicate everywhere.

**Interconnect Defense**: Server doesn't accept state from peers. No replication flood.

### State Resolution DoS

**Matrix Risk**: Craft complex conflicting events to burn CPU resolving "truth".

**Interconnect Defense**: No state resolution. One server, one truth.

## Remaining Attack Surface

### Transfer Passport Manipulation

**Attack**: Edit passport to claim items/abilities you don't have.

**Defense**: Import policies. Destination server validates and sanitizes:

```rust
fn on_player_enter(passport: Passport) -> Player {
    let mut player = Player::new();

    // Sanitize stats
    player.health = passport.health.clamp(0, MAX_HEALTH);
    player.level = passport.level.clamp(1, MAX_LEVEL);

    // Filter inventory
    for item in passport.items {
        if self.allowed_items.contains(&item.id) {
            player.give(item);
        } else {
            self.notify("Contraband confiscated: {}", item.name);
        }
    }

    player
}
```

### Substrate Poisoning

**Attack**: Serve malicious substrate data to poison caches.

**Defense**: Content addressing. Substrate is identified by hash. Verify before caching.

### Authority Impersonation

**Attack**: Claim to be the authority for a world you don't own.

**Defense**: World ownership is signed. Clients verify server identity against known registry.

## Trust Model

- Clients trust the current authority (unavoidable for real-time games)
- Authorities don't trust clients (intent-only protocol)
- Authorities don't trust each other (import policies)
- Substrate is trustless (content-addressed)
