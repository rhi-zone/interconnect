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

## Context Is The Only Scarce Resource

Every byte that enters the main session stays in the main session for its entire lifetime. File contents, command output, search results — once read, it lingers in cache and shapes every downstream token. There is no "just looking."

**All exploration runs in subagents. No exceptions.** Any tool call whose purpose is "find out what's here" — grep, find, broad reads, surveys, audits — belongs in a subagent. Raw exploratory output in the main context is active context poisoning. The subagent returns a distilled summary; the noise stays in the subagent.

Inline tool use in the main context is reserved for:
- Reading a known file at a known path
- Edits and writes you're committing to
- A single targeted lookup whose result you'll act on immediately

If you find yourself running a second grep to refine the first, you should have spawned a subagent.

## Subagent Prompts

A subagent prompt is composed in a "spec-writing" register that subtly changes what feels in-scope. Specific failure modes to name:

**Never tell a subagent "do not commit."** Delegation does not strip the commit step from completed work. If a subagent modifies files and the work is done, either the subagent commits, or the next thing the delegator does after it returns is commit — not summarize, not report. The phrase "do not commit" in your own prompt is the tell that you are about to leave work uncommitted.

**Do not delegate judgment.** Phrases like "if extraction is awkward, just duplicate" or "based on your findings, fix the bug" push synthesis onto the agent. If you are punting a decision into the prompt, you do not yet have enough understanding to delegate. Investigate first; write the prompt with the decision already made.

**Do not ask for a diff summary.** Subagent self-reports describe intent, not effect. After a code-modifying subagent returns, read `git diff` yourself. Skip the "report what you changed" instruction — it produces text you cannot trust and that pollutes main context.

**Do not re-explain CLAUDE.md.** Subagents inherit it. Repeating project layout or repo conventions in the prompt dilutes the actual task instructions and signals half-trust in the inheritance. Trust it or don't read it.

**Line numbers are orientation, not anchors.** Files shift between your read and the subagent's read. When citing locations, tell the subagent to find the lines by content ("the block that does X"), not by number.

**Name files explicitly; do not outsource the grep.** "Wherever it appears" invites scope creep. Grep first, list the exact files in the prompt.

**If the task is smaller than the prompt describing it, do it inline.** A subagent dispatch pays a full system-prompt + CLAUDE.md cache cost. One-shot bash commands and single-line edits should run in the main session with `Bash` or `Edit`.

**Match agent type to deliverable shape.** `Explore` is for lookup and search — finding files, symbols, references — not analytical synthesis. For audits, surveys, and pattern analysis whose deliverable is a report, use `general-purpose` with an explicit Opus model. For tasks whose deliverable is files on disk, use `general-purpose` with the tier matched to the work (Sonnet for mechanical, Opus for architectural).

**On unsatisfying subagent output, change something before retrying.** Same prompt + same model + same agent type = same result. Escalate model tier (Sonnet → Opus), narrow the prompt, or switch agent type. Identical retries are waste.

**Dispatch independent subagents in parallel.** Multiple Agent tool_use blocks in a single assistant message run concurrently. Serial Agent dispatch across sequential turns is the default failure mode and trades wall time for nothing. If two subagents do not depend on each other's output, they belong in the same message.

**Pair `isolation: worktree` with `run_in_background: true`.** A worktree implies meaningful write work. Foregrounding it blocks the main session for the entire run. Background unless the worktree's immediate output is what you need to act on next.

**Always set `subagent_type` and `model` explicitly.** Defaulting either collapses tier choice into an invisible decision. The model and agent type are part of the spec; name them every time, even when the choice is obvious. See the existing `Subagent model tiers` section above for which tier fits which work.

## Durability

Subagent reports, mid-session realizations, "I'll remember this" — none of these outlast the session. Anything worth keeping goes into CLAUDE.md, code, docs, or a commit. If it isn't written down, it is gone.

**Commit completed work immediately.** After tests pass, commit. After each phase of a multi-phase plan, commit. Uncommitted work is lost work, and accumulated uncommitted phases lose isolation as well.

**Docs change in the same commit as the code.** New pages enter the sidebar in that commit. There is no follow-up.

## Authenticity

When asked to analyze X, read X. Do not synthesize from conversation memory, prior summaries, or what the file probably says. Claims must correspond to evidence produced this session.

**Something unexpected is a signal.** Surprising output, anomalous numbers, a file containing what it shouldn't — stop and find out why. Do not accept the anomaly and proceed.

## Discipline

Corrections from the user are conversation, not material for new rules. A single correction does not warrant a CLAUDE.md edit. Rules are added when a failure mode is observed repeatedly and the rule names the failure it prevents.

Do not announce actions ("I will now…"). Act.

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
