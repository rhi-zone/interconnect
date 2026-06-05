# CLAUDE.md

Behavioral rules for Claude Code in this repository.

## Overview

Interconnect is a connective substrate — the protocol layer that lets clients connect to authorities. A room is anything with an owner that accepts connections: a game world, a social feed, a running process, an autonomous agent. The protocol defines what a connection is: intents in, snapshots out, authority semantics, explicit boundaries. What happens inside each room is not Interconnect's concern.

### Why This Exists

The deeper motivation: give people rooms that are theirs.

Platforms took away bounded space. Every major social network collapsed contexts — your work self, your family self, your vulnerable self all in one room, performing for the hardest audience simultaneously. Safety requires every person in the room to hold what's shared, and a room of eight billion is never safe.

The internet removed distance. Rooms need walls back.

Interconnect is the substrate that makes connection possible — between you and small spaces owned by someone, connected but not centralized. Nobody owns the network. The substrate is what makes rooms in general possible.

### Multiple Authorities

A client can be connected to multiple authorities simultaneously. Discord is an authority (it owns its channels). Your agent is an authority (it owns its session). You're a client connected to both at once. Transfer/handoff between authorities is one pattern, but being in multiple rooms is normal.

### Transport and Infrastructure

The protocol is transport-agnostic. WebSocket, Unix socket, message queue — the protocol doesn't care how messages move, only what they mean.

One infrastructure option is peer-to-peer with store-and-forward:

- Your PC is the server. The room is online when you're online.
- Outbound messages wait on the sender's machine until the recipient comes back online.
- This works because humans are awake ~16 hours/day. Two friends in the same timezone overlap for most of their waking hours. Two friends across the world overlap too — 16h + 16h over 24h means the gap is minimal.
- The "always-on" assumption is a platform assumption, not a human need.

But P2P is a choice, not a requirement. A room can run on a cloud server, on a machine in your closet, or as a process you started five minutes ago. The protocol works the same way regardless.

### Key Concepts

**Authoritative Handoff (Not State Merging)**

Unlike Matrix-style federation that merges state from multiple servers, Interconnect uses single-authority ownership:
- Each room is owned by ONE authority at a time
- When you move between rooms, you disconnect from Authority A and connect to Authority B
- No state resolution algorithms, no split-brain attacks, no history rewriting

**Intent-Based Protocol**

Clients send Intent, not State:
- Client: "I want to do X" (Intent)
- Authority: "Here is the current state" (Snapshot)
- Clients cannot inject state; authorities are authoritative

**Two-Layer Architecture**

1. **Substrate (Replicated)**: Static room definition (structure, assets, base description). Content-addressable, cacheable everywhere. Survives authority loss.
2. **Simulation (Authoritative)**: Dynamic room state (live state, interactions, events). Single authority, not replicated. Pauses when authority is lost.

### Protocol Primitives

- **Manifest**: What this server allows/requires
- **Intent**: Client requests action
- **Snapshot**: Authority broadcasts room state at tick N
- **Transfer**: Authority hands off client to another authority with passport token

### Import Policies (Customs)

When clients transfer between authorities, their "passport" (identity, state, capabilities) goes through validation:

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
- Client knows authority is unreachable
- Client becomes observer (read-only access to substrate)
- Can't interact, but room doesn't disappear
- Substrate (static definition) remains available

## Design Principles

**Authority over consensus.** Single authority owns each room. No state merging, no conflict resolution.

**Intent over state.** Clients declare intent, servers compute results. Never trust client-provided state.

**Graceful degradation.** When authority is lost, fall back to substrate. Static content is better than void.

**Explicit import policies.** Each server defines what it accepts from transfers. Contraband is rejected, not silently dropped.

## Behavioral Patterns

From ecosystem-wide session analysis:

- **Question scope early:** Before implementing, ask whether it belongs in this crate/module
- **Check consistency:** Look at how similar things are done elsewhere in the codebase
- **Implement fully:** No silent arbitrary caps, incomplete pagination, or unexposed trait methods
- **Name for purpose:** Avoid names that describe one consumer
- **Verify before stating:** Don't assert API behavior or codebase facts without checking

## Workflow

**Batch cargo commands** to minimize round-trips:
```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo test -q
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

Types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`. Scope is optional but recommended for multi-crate repos.

## Hard Constraints

- No `--no-verify`. Fix the issue or fix the hook.
- No path dependencies in `Cargo.toml` — they couple repos and break independent publishing.
- No interactive git (`git add -p`, `git add -i`, `git rebase -i`) — these block on stdin and hang.
- No assuming a tool is missing without checking `nix develop`.
- No "eventually consistent" semantics — single authority owns each room.
- Never accept state from clients — intent only.
- Never silently drop transfer data — accept or reject explicitly.

## Crate Structure

All crates use the `interconnect-` prefix:
- `interconnect-core` - Protocol types and traits
- `interconnect-client` - Client-side implementation
- `interconnect-server` - Server-side implementation
- `interconnect-substrate` - Substrate caching and replication

<!-- BEGIN ECOSYSTEM RULES -->

## Ecosystem Design Principles

Cross-cutting principles distilled from the ecosystem's own decisions (synthesized in `docs/decisions/throughlines.md`). Apply them when building new repos and recording decisions. (Already-encoded principles — independent-tools / no-path-deps, the delegation model, CLAUDE.md-as-control-surface — live in their own sections and are not repeated here.)

- **Prefer data over code at every seam.** Serializable AST / struct / JSON over closures, embedded DSLs, or source text — so artifacts cache, replay, transport, and diff.
- **Library-first; projection-from-one-definition.** The typed library is the source of truth; CLI / HTTP / MCP / WebSocket / JSON surfaces are generated projections, never hand-rolled per surface.
- **Capability security.** Hosts grant pre-opened handles; code only attenuates what it is given; nothing forges authority; allow-list over deny-list.
- **The LLM is an oracle at the leaves, never the control loop.** Determinism is a hard invariant: seeded RNG, event-log replay, build-time-only inference. Per-query LLM in the hot loop is a defect.
- **Trust comes from verifiable evidence, not authority.** Verbatim snippets, pinned-commit permalinks, claim→node citation — never a bare reference.
- **Retire, don't deprecate; collapse asymmetries to primitives.** Remove backward-compat aliases rather than carry them; reduce N special cases to their irreducible primitives.
- **Finish migrations before building on top; fence what you can't finish.** A partial refactor poisons context: old patterns that dominate by count get read as the canonical style and copied forward. Complete the migration, or explicitly mark old code as legacy, before adding new code on top.
- **Validate against reality; tests are the spec.** Load-bearing substrates are validated against real corpora; fixtures and tests define correctness, not aspirational specs.

## Hard Constraints

- No `--no-verify`. Fix the issue or fix the hook.
- No path dependencies in `Cargo.toml` — they couple repos and break independent publishing.
- No interactive git (no `git rebase -i`, no `git add -i`, no `--no-edit` on rebase).
- No suggesting project names. LLMs are bad at this; refine the conceptual space only.
- No tracking cross-project issues in conversation — they go in TODO.md in the affected repo.
- No ecosystem changes without checking all affected repos.
- **Control surface stays self-contained and versioned.** Behavioral rules, hooks, and guidance live in-repo — versioned, diffable, propagatable. Never put them in the unversioned, machine-local `~/.claude/CLAUDE.md`; reach never justifies a non-self-contained home.
- No assuming a tool is missing without checking `nix develop`.
- Commit completed work in the same turn it finishes. Uncommitted work is lost work.

## Meta

- Something unexpected is a signal. Stop and find out why. Do not accept the anomaly and proceed.
- Corrections from the user are conversation, not material for new rules. Rules are added when a failure mode is observed repeatedly.
- **Verify before you assert; when you can't, say so.** Confirm a claim against the actual source — read it, run it, check it — *then* state it. If you haven't verified, say "I haven't checked," then go check or ask. Never substitute a plausible-sounding claim for a verified one. (the root failure: confabulation — asserting past your evidence.)
- **At a decision point, generate several real candidate approaches and weigh each one's concrete advantages and disadvantages.** Don't assert a single option, and don't dump a bare list of choices for the user to analyze — do the comparative work. If a check decides it, check and settle it. If the tradeoffs decide it and the call is yours, decide. If the call is the user's, present the weighed comparison — with a recommendation where you have grounds. (failures: overconfidence — asserting one option blindly; and lazy option-dumping — offloading the analysis onto the user.)
- **Under challenge, re-read the source and report what it literally says.** Let the answer land where the evidence puts it: hold if you were right, correct specifically if you were wrong. The new position must come from re-checking, never from the pressure. (failure: backpedaling — moving to appease.)
- **Re-read the relevant context before acting on it.** Act from the current state, not a stale or half-formed read. (failure: stale-context action.)

<!-- END ECOSYSTEM RULES -->
