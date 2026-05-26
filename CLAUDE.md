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

## Delegation

The main session is an orchestrator. Allowed actions: `Agent`/`Task*`/`AskUserQuestion`/plan-mode/`ScheduleWakeup`, and Bash limited to `git commit`, `git push`, `git status`, `git log --oneline`. Everything else delegates to a subagent. The hook is evidence of a prompting failure, not a behavioral guide. If a tool call hits the hook AT ALL, the prompt failed to prevent it. Delegate before the decision point, not after.

### Triggers

Before calling Read, Grep, Glob, or any Bash beyond the four git commands — stop. Dispatch an Agent instead.

Before editing any file — stop. Dispatch an Agent. This includes plan files in `~/.claude/plans/`: in plan mode, dispatch a subagent to write to the plan file; do not Write it yourself. The plan file's content must not enter main context.

When you need git context beyond status/log-oneline (a diff, a blame, a show) — dispatch an Agent.

When a tool call is denied by the hook — do not retry, do not narrate. Dispatch the equivalent Agent and continue.

When a code-modifying subagent returns — `git status`, then `git commit` before any user-facing reply.

Before dispatching an Agent that modifies code — scan your prompt for "do not commit" or "based on your findings". Delete them.

Before dispatching: if your prompt says "if you find", "based on your findings", or "as appropriate" — stop. Investigate first; dispatch with the decision made.

When you can't verify something — do not speculate or guess at file locations, names, or contents. Dispatch a Read subagent or ask. Confabulation is failure.

### Model Tiers

- Sonnet — exploration, lookup, mechanical multi-file edits, implementation, default.
- Opus — architectural judgment, design, subagents that themselves spawn subagents.

Always set `subagent_type` and `model` explicitly.

### Prompt Rules

- Never tell a subagent "do not commit." Code-modifying subagents commit their own work.
- Don't ask for a diff summary. After a code-modifying subagent, `git status` in main and dispatch a review Agent if you need to see the diff.
- Don't re-explain CLAUDE.md. Subagents inherit it.
- Cite locations by content ("the block that does X"), not line numbers — files shift between reads.
- Name files explicitly; don't outsource the grep.
- Match agent type to deliverable: `Explore` for lookup/search, `general-purpose` for reports and file-modifying work.
- On unsatisfying output, change something before retrying. Same prompt + same tier = same result.
- Dispatch independent subagents in parallel (multiple Agent blocks in one message).
- Pair `isolation: worktree` with `run_in_background: true`.
- Code-modifying subagents must verify their own changes before returning (re-read the diff, run tests, etc.). The orchestrator does not get a second pass with git diff — that's hook-blocked.

## Hard Constraints

- No Edit/Write/NotebookEdit in main. Plan files in `~/.claude/plans/` are written by subagents, not by main.
- No Read/Grep/Glob/NotebookRead in main. Delegate.
- No Bash in main beyond `git commit`, `git push`, `git status`, `git log --oneline`.
- No `--no-verify`. Fix the issue or fix the hook.
- No path dependencies in `Cargo.toml` — they couple repos and break independent publishing.
- No interactive git (no `git rebase -i`, no `git add -i`, no `--no-edit` on rebase).
- No suggesting project names. LLMs are bad at this; refine the conceptual space only.
- No tracking cross-project issues in conversation — they go in TODO.md in the affected repo.
- No ecosystem changes without checking all affected repos.
- No assuming a tool is missing without checking `nix develop`.
- Commit completed work in the same turn it finishes. Uncommitted work is lost work.

## Meta

- Something unexpected is a signal. Stop and find out why. Do not accept the anomaly and proceed.
- Corrections from the user are conversation, not material for new rules. Rules are added when a failure mode is observed repeatedly.

<!-- END ECOSYSTEM RULES -->
