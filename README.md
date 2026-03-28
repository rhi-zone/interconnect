# Interconnect

Connective substrate for authoritative rooms.

Part of the [Rhi](https://rhi.zone) ecosystem.

## Overview

Interconnect is the protocol layer that lets clients connect to authorities. A room is anything with an owner that accepts connections — a game world, a social feed, a running process, an autonomous agent. The protocol defines what a connection is: intents in, snapshots out, authority semantics, explicit boundaries.

Transport-agnostic. The protocol doesn't care how messages move, only what they mean.

## Key Ideas

### Authority over consensus

Each room is owned by one authority. No state merging, no distributed consensus. When you move between rooms, you disconnect from Authority A and connect to Authority B.

### Intent-based protocol

Clients send what they want to do, not what happened:

```
Client → Authority: Intent { ... }
Authority → Client: Snapshot { ... }
```

What intents and snapshots contain is defined by the room. A game room sends movement intents and entity state. An agent room sends steering messages and progress snapshots.

### Two-layer architecture

1. **Substrate** — Static room definition. Replicated, cacheable, survives authority loss.
2. **Simulation** — Live state. Single authority, ephemeral.

When an authority goes down, the substrate remains (ghost mode). The room pauses, it doesn't disappear.

## Protocol Primitives

| Primitive | Direction | Purpose |
|-----------|-----------|---------|
| Manifest | Authority → Client | What this room allows/requires |
| Intent | Client → Authority | Request an action |
| Snapshot | Authority → Client | Room state at tick N |
| Transfer | Authority → Client | Handoff to another authority |

## License

MIT
