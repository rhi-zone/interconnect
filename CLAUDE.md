# CLAUDE.md

Behavioral rules for Claude Code in this repository.

## Overview

Interconnect is a connective substrate — the protocol layer that lets clients connect to authorities. A room is anything with an owner that accepts connections: a game world, a social feed, a running process, an autonomous agent. The protocol defines what a connection is: intents in, snapshots out, authority semantics, explicit boundaries. What happens inside each room is not Interconnect's concern.

### Why This Exists

The deeper motivation: give people rooms that are theirs.

Platforms took away bounded space. Every major social network collapsed contexts — your work self, your family self, your vulnerable self all in one room, performing for the hardest audience simultaneously. Safety requires every person in the room to hold what's shared, and a room of eight billion is never safe.

The internet removed distance. Rooms need walls back.

Interconnect is the substrate that makes connection possible — between you and small spaces owned by someone, connected but not centralized. Nobody owns the network. The substrate is what makes rooms in general possible.

### Transport and Infrastructure

The protocol is transport-agnostic. WebSocket, Unix socket, Discord bot, message queue — the protocol doesn't care how messages move, only what they mean.

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

## Core Rules

**Note things down immediately — no deferral:**
- Problems, tech debt, issues → TODO.md now, in the same response
- Design decisions, key insights → docs/ or CLAUDE.md
- Future/deferred scope → TODO.md **before** writing any code, not after
- **Every observed problem → TODO.md. No exceptions.** Code comments and conversation mentions are not tracked items. If you write a TODO comment in source, the next action is to open TODO.md and write the entry.

**Conversation is not memory.** Anything said in chat evaporates at session end. If it implies future behavior change, write it to CLAUDE.md or a memory file immediately — or it will not happen.

**Warning — these phrases mean something needs to be written down right now:**
- "I won't do X again" / "I'll remember to..." / "I've learned that..."
- "Next time I'll..." / "From now on I'll..."
- Any acknowledgement of a recurring error without a corresponding CLAUDE.md or memory edit

**Triggers:** User corrects you, 2+ failed attempts, "aha" moment, framework quirk discovered → document before proceeding.

**When the user corrects you:** Ask what rule would have prevented this, and write it before proceeding. **"The rule exists, I just didn't follow it" is never the diagnosis** — a rule that doesn't prevent the failure it describes is incomplete; fix the rule, not your behavior.

**Something unexpected is a signal, not noise.** Surprising output, anomalous numbers, files containing what they shouldn't — stop and ask why before continuing. Don't accept anomalies and move on.

**Do the work properly.** When asked to analyze X, actually read X - don't synthesize from conversation.

## Behavioral Patterns

From ecosystem-wide session analysis:

- **Question scope early:** Before implementing, ask whether it belongs in this crate/module
- **Check consistency:** Look at how similar things are done elsewhere in the codebase
- **Implement fully:** No silent arbitrary caps, incomplete pagination, or unexposed trait methods
- **Name for purpose:** Avoid names that describe one consumer
- **Verify before stating:** Don't assert API behavior or codebase facts without checking

## Design Principles

**Authority over consensus.** Single authority owns each room. No state merging, no conflict resolution.

**Intent over state.** Clients declare intent, servers compute results. Never trust client-provided state.

**Graceful degradation.** When authority is lost, fall back to substrate. Static content is better than void.

**Explicit import policies.** Each server defines what it accepts from transfers. Contraband is rejected, not silently dropped.

## Workflow

**Batch cargo commands** to minimize round-trips:
```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo test -q
```
After editing multiple files, run the full check once — not after each edit. Formatting is handled automatically by the pre-commit hook (`cargo fmt`).

**When making the same change across multiple crates**, edit all files first, then build once.

**Minimize file churn.** When editing a file, read it once, plan all changes, and apply them in one pass. Avoid read-edit-build-fail-read-fix cycles by thinking through the complete change before starting.

**Always commit completed work.** After tests pass, commit immediately — don't wait to be asked. When a plan has multiple phases, commit after each phase passes. Do not accumulate changes across phases. Uncommitted work is lost work.

**Use `normalize view` for structural exploration:**
```bash
~/git/rhizone/normalize/target/debug/normalize view <file>    # outline with line numbers
~/git/rhizone/normalize/target/debug/normalize view <dir>     # directory structure
```

## Context Management

**Use subagents to protect the main context window.** For broad exploration or mechanical multi-file work, delegate to an Explore or general-purpose subagent rather than running searches inline. The subagent returns a distilled summary; raw tool output stays out of the main context.

Rules of thumb:
- Research tasks (investigating a question, surveying patterns) → subagent; don't pollute main context with exploratory noise
- Searching >5 files or running >3 rounds of grep/read → use a subagent
- Codebase-wide analysis (architecture, patterns, cross-file survey) → always subagent
- Mechanical work across many files (applying the same change everywhere) → parallel subagents
- Single targeted lookup (one file, one symbol) → inline is fine

## Session Handoff

Use plan mode as a handoff mechanism when:
- A task is fully complete (committed, pushed, docs updated)
- The session has drifted from its original purpose
- Context has accumulated enough that a fresh start would help

**For handoffs:** enter plan mode, write a short plan pointing at TODO.md, and ExitPlanMode. **Do NOT investigate first** — the session is context-heavy and about to be discarded. The fresh session investigates after approval.

**For mid-session planning** on a different topic: investigating inside plan mode is fine — context isn't being thrown away.

Before the handoff plan, update TODO.md and memory files with anything worth preserving.

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
- Use interactive git commands (`git add -p`, `git add -i`, `git rebase -i`) — these block on stdin and hang in non-interactive shells; stage files by name instead
- Design for "eventually consistent" semantics
- Accept state from clients
- Silently drop transfer data - either accept or reject explicitly
- Require all servers to trust each other
- Use path dependencies in Cargo.toml - causes clippy to stash changes across repos
- Use `--no-verify` - fix the issue or fix the hook
- Assume tools are missing - check if `nix develop` is available for the right environment

## Crate Structure

All crates use the `interconnect-` prefix:
- `interconnect-core` - Protocol types and traits
- `interconnect-client` - Client-side implementation
- `interconnect-server` - Server-side implementation
- `interconnect-substrate` - Substrate caching and replication
