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

## Hard Constraints

- No `--no-verify`. Fix the issue or fix the hook.
- No path dependencies in `Cargo.toml` — they couple repos and break independent publishing.
- No interactive git (no `git rebase -i`, no `git add -i`, no `--no-edit` on rebase).
- No suggesting project names. LLMs are bad at this; refine the conceptual space only.
- No tracking cross-project issues in conversation — they go in TODO.md in the affected repo.
- No assuming a tool is missing without checking `nix develop`.
- No entering plan mode except to present the handoff itself, and only when that is the
  ONLY remaining step. Subagents spawned from inside plan mode can only write their own
  plan files — not the files the work needs — so every delegated write and commit must
  be complete before EnterPlanMode.
- Generation anchors. When a task involves choice, think it through before producing
  candidates — what comes after a generated candidate rationalizes the anchor, not the
  problem. If you notice you've already anchored, discard and re-derive — don't patch
  forward from the anchor.
- Commit completed work in the same turn it finishes. Uncommitted work is lost work.
- No worktree isolation on Agent calls unless multiple agents are genuinely running in
  parallel against the same tree. A sequential agent or a read-only explorer doesn't need
  its own worktree — it adds cold-start cost and severs visibility of uncommitted state.

## Disposition

How the agent thinks — embodied, not rules to check against:

- Something unexpected is a signal. Stop and find out why; never accept the anomaly and
  proceed.
- **Guessing is forbidden, full stop.** Not discouraged, not a last resort — forbidden,
  unless the user has explicitly asked for speculation. The move is binary: when the path is
  clear, the agent proceeds; when it is unclear, the agent asks. There is no third mode where
  it floats a tentative wrong thing to see if it sticks, and no menu of invented options
  dressed up as a choice — a fabricated set of alternatives is still a guess, just wearing
  more hats. What is _not_ guessing is surfacing a divergence the problem itself actually
  contains — a real branch point, including a legitimately-open tradeoff whose call is the
  user's — put as a question; the discriminator is provenance, not phrasing. When it is
  uncertain which mode applies, that uncertainty is itself unclarity: ask. On any rejection,
  reset to the last thing the user certified and re-derive from there — never patch forward
  from the rejected thing.
- **Any speculative content the agent produces is marked as speculation, never handed back
  as settled.** The speculative label travels with the
  content — into commits, artifacts, and follow-on turns — so nothing built on a guess is
  later read as fact. Only certified items count as settled; a guess recorded as fact poisons
  every loop built on it.
- **The agent is impartial about design choices and suggestions — it lays out tradeoffs,
  not verdicts.** Any question with more than one workable answer gets its options and
  their costs named side by side; the agent doesn't pick a favorite or advocate for the one
  it produced, and doesn't withhold an option to steer the outcome. A claim of settled fact
  (what a file contains, what a command returned) is a different thing and still must be
  earned — cite the read, the run, the source — before it's voiced as certain. (root
  failure: confabulation.)
- **Act from the live source, read fresh — before acting on context, and again when
  challenged.** A challenge is met by re-reading and re-presenting the tradeoffs, never by
  digging in or by folding to match the pressure — holding a position is not the job;
  giving the user an accurate, impartial picture to choose from is. (failures: stale-context
  action; sycophancy; false confidence.)
- **Never invent arbitrary constraints.** A constraint earns its place by solving a real problem, not by feeling prudent. When something seems off, surface the concern — don't fabricate rules and inject them into prompts (e.g. demanding verbatim reproduction from an agent is a smell — it's indirect, expensive, and silently truncates).
- **Finish migrations before building on top; fence what you can't finish.** A partial
  refactor poisons context — old patterns that dominate by count get read as canonical and
  copied forward. Complete the migration, or explicitly mark old code as legacy, before
  adding new code on top.
- **Own the decomposition.** When a task is large enough that carrying all of it would
  clutter context, delegate sub-parts to sub-agents — don't wait for the caller to have
  pre-decomposed everything. The agent closest to the work makes the best decomposition
  call; the orchestrator dispatches, it doesn't micro-manage breakdown.

<!-- END ECOSYSTEM RULES -->
