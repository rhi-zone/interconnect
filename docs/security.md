# Security Model

## Eliminated Attack Classes

By using authoritative handoff instead of state resolution:

### History Rewrite Attack

**Matrix Risk**: Inject fake events into the past to change current state.

**Interconnect Defense**: Impossible. Clients cannot send state, only intent. Authority computes all results.

### Split-Brain Attack

**Matrix Risk**: Partition network, create two realities, merge chaos.

**Interconnect Defense**: If Authority A goes offline, the room pauses. You can't fork the room because Authority A is the only machine with the valid simulation.

### State Bloom Attack

**Matrix Risk**: Flood room with metadata updates, replicate everywhere.

**Interconnect Defense**: Authority doesn't accept state from peers. No replication flood.

### State Resolution DoS

**Matrix Risk**: Craft complex conflicting events to burn CPU resolving "truth".

**Interconnect Defense**: No state resolution. One authority, one truth.

## Remaining Attack Surface

### Transfer Passport Manipulation

**Attack**: Edit passport to claim capabilities or items you don't have.

**Defense**: Import policies. Destination authority validates and sanitizes:

```rust
fn on_client_enter(passport: Passport) -> ClientState {
    let mut state = ClientState::new();

    // Sanitize numeric fields
    state.credits = passport.credits.clamp(0, MAX_CREDITS);
    state.level = passport.level.clamp(1, MAX_LEVEL);

    // Filter carried items
    for item in passport.items {
        if self.allowed_items.contains(&item.id) {
            state.give(item);
        } else {
            self.notify("Contraband confiscated: {}", item.name);
        }
    }

    state
}
```

### Substrate Poisoning

**Attack**: Serve malicious substrate data to poison caches.

**Defense**: Content addressing. Substrate is identified by hash. Verify before caching.

### Authority Impersonation

**Attack**: Claim to be the authority for a room you don't own.

**Defense**: Room ownership is signed. Clients verify authority identity against known registry.

## Trust Model

- Clients trust the current authority (unavoidable for live, authoritative rooms)
- Authorities don't trust clients (intent-only protocol)
- Authorities don't trust each other (import policies)
- Substrate is trustless (content-addressed)
