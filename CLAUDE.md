# CLAUDE.md

Behavioral rules for Claude Code in this repository.

## Overview

Interconnect is a federation protocol for persistent worlds. It enables Lotus servers to form interconnected networks where players can travel between worlds owned by different authorities.

### Key Concepts

**Authoritative Handoff (Not State Merging)**

Unlike Matrix-style federation that merges state from multiple servers, Interconnect uses single-authority ownership:
- Each world/room is owned by ONE server at a time
- When you move between worlds, you disconnect from Server A and connect to Server B
- No state resolution algorithms, no split-brain attacks, no history rewriting

**Intent-Based Protocol**

Clients send Intent, not State:
- Client: "I want to move north" (Intent)
- Server: "You are now at (5, 3)" (Snapshot)
- Clients cannot inject state; servers are authoritative

**Two-Layer Architecture**

1. **Substrate (Replicated)**: Static world definition (geometry, textures, base description). Content-addressable, cacheable everywhere. Survives server death.
2. **Simulation (Authoritative)**: Dynamic world state (physics, player positions, door states). Single server, not replicated. Pauses when server dies.

### Protocol Primitives

- **Manifest**: What this server allows/requires
- **Intent**: Client requests action
- **Snapshot**: Server broadcasts world state at tick N
- **Transfer**: Server hands off player to another server with passport token

### Import Policies (Customs)

When players transfer between servers, their "passport" (inventory, stats) goes through validation:

```rust
fn on_player_enter(passport: Spore) -> Player {
    let mut player = Player::new();
    player.health = passport.health.clamp(0, 100);
    for item in passport.items {
        if self.allowed_items.contains(&item.id) {
            player.give(item);
        }
    }
    player
}
```

### Ghost Mode

When authority connection is lost:
- World desaturates / shows static effect
- Player becomes observer (client-side collision only)
- Can't interact, but world doesn't disappear
- Substrate (static world) remains visible

## Core Rule

**Note things down immediately:**
- Bugs/issues → fix or add to TODO.md
- Design decisions → docs/ or code comments
- Future work → TODO.md
- Key insights → this file

**Do the work properly.** When asked to analyze X, actually read X - don't synthesize from conversation.

## Behavioral Patterns

From ecosystem-wide session analysis:

- **Question scope early:** Before implementing, ask whether it belongs in this crate/module
- **Check consistency:** Look at how similar things are done elsewhere in the codebase
- **Implement fully:** No silent arbitrary caps, incomplete pagination, or unexposed trait methods
- **Name for purpose:** Avoid names that describe one consumer
- **Verify before stating:** Don't assert API behavior or codebase facts without checking

## Design Principles

**Authority over consensus.** Single server owns each world. No state merging, no conflict resolution.

**Intent over state.** Clients declare intent, servers compute results. Never trust client-provided state.

**Graceful degradation.** When authority dies, fall back to substrate. Static world is better than void.

**Explicit import policies.** Each server defines what it accepts from transfers. Contraband is rejected, not silently dropped.

## Workflow

**Batch cargo commands** to minimize round-trips:
```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo test
```
After editing multiple files, run the full check once — not after each edit. Formatting is handled automatically by the pre-commit hook (`cargo fmt`).

**When making the same change across multiple crates**, edit all files first, then build once.

**Use `normalize view` for structural exploration:**
```bash
~/git/rhizone/normalize/target/debug/normalize view <file>    # outline with line numbers
~/git/rhizone/normalize/target/debug/normalize view <dir>     # directory structure
```

## Commit Convention

Use conventional commits: `type(scope): message`

Types:
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code change that neither fixes a bug nor adds a feature
- `docs` - Documentation only
- `chore` - Maintenance (deps, CI, etc.)
- `test` - Adding or updating tests

Scope is optional but recommended for multi-crate repos.

## Negative Constraints

Do not:
- Announce actions ("I will now...") - just do them
- Leave work uncommitted
- Design for "eventually consistent" semantics
- Accept state from clients
- Silently drop transfer data - either accept or reject explicitly
- Require all servers to trust each other
- Use path dependencies in Cargo.toml - causes clippy to stash changes across repos
- Use `--no-verify` - fix the issue or fix the hook
- Assume tools are missing - check if `nix develop` is available for the right environment

## Crate Structure

All crates use the `rhi-interconnect-` prefix:
- `rhi-interconnect-core` - Protocol types and traits
- `rhi-interconnect-client` - Client-side implementation
- `rhi-interconnect-server` - Server-side implementation
- `rhi-interconnect-substrate` - Substrate caching and replication
