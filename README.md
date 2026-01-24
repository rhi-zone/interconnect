# Interconnect

Federation protocol for persistent worlds.

Part of the [Rhi](https://rhi.zone) ecosystem.

## Overview

Interconnect enables Lotus servers to form interconnected networks. Players can travel between worlds owned by different authorities without the complexity of distributed state resolution.

## Key Ideas

### Authoritative Federation

Unlike Matrix-style federation that tries to merge state from multiple servers, Interconnect uses single-authority ownership:

- Each world is owned by ONE server
- Moving between worlds = disconnect from A, connect to B
- No split-brain attacks, no state resolution DoS

### Intent-Based Protocol

Clients send what they want to do, not what happened:

```
Client → Server: RequestMove { direction: North }
Server → Client: Snapshot { position: (5, 3), tick: 500 }
```

### Two-Layer Architecture

1. **Substrate** - Static world definition. Replicated, cacheable, survives server death.
2. **Simulation** - Dynamic state. Single authority, ephemeral.

When a server goes down, you can still explore the world (substrate). You just can't interact (simulation).

## Protocol Primitives

| Primitive | Direction | Purpose |
|-----------|-----------|---------|
| Manifest | Server → Client | What this server allows/requires |
| Intent | Client → Server | Request an action |
| Snapshot | Server → Client | World state at tick N |
| Transfer | Server → Client | Handoff to another server |

## License

MIT
