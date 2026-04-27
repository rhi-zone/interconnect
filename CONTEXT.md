# Ubiquitous Language

Domain vocabulary for Interconnect. Use these terms precisely in code, docs, and conversations.

## Authority
_Avoid:_ server, host, node

The single source of truth for a room's simulation state. An authority owns the room — it processes intents, advances simulation, and broadcasts snapshots. There is exactly one authority per room at any time.

Confusing with "server" loses the ownership semantics: a server might host many rooms across many authorities, or hand off authority via transfer.

## Room
_Avoid:_ channel, session, space

A bounded context with a single authority that clients connect to. Can represent a game world, a social feed, a process, or an agent — the abstraction is ownership and connection, not the content.

Confusing with "session" inverts the relationship: a session is a client-authority pair, not the room itself.

## Session
_Avoid:_ connection, client

A connected client-authority pair: carries session ID, identity, and display name. Sessions belong to rooms; rooms are not sessions.

## Substrate
_Avoid:_ state, content, data

The static, replicated definition of a room: structure, assets, base description. Content-addressable and cacheable; survives authority loss. The substrate is what remains accessible in ghost mode.

Confusing substrate with simulation collapses the key architectural distinction: substrate is stable and replicated, simulation is live and authority-dependent.

## Simulation
_Avoid:_ state, world state

The dynamic, authoritative room state: live interactions, events, mutable data. Ephemeral — it exists only while an authority is running and connected. Clients cannot access simulation in ghost mode.

## Intent
_Avoid:_ command, action, event, request

A client-sent message declaring what the client wants to do. Application-defined. Intents express desired action, not state changes — the authority decides what actually happens.

Confusing with "event" reverses causality: events are things that happened, intents are things the client is asking to happen.

## Snapshot
_Avoid:_ state update, broadcast, sync

Authoritative state broadcast from server to clients at a given sequence number. The server's word on what the simulation currently looks like. Clients apply snapshots to stay synchronized.

## Manifest
_Avoid:_ config, metadata, descriptor

Server metadata describing capabilities, requirements, substrate hash, and app-defined metadata. Clients read the manifest to understand what a room offers before connecting.

## Passport
_Avoid:_ token, credentials, identity

Identity plus app-defined data that travels with a client during a transfer. Subject to import policy validation by the receiving authority. Think of it as what the client carries across the border.

## Transfer
_Avoid:_ redirect, migration, reconnect

A server-to-server handoff: client disconnects from Authority A and connects to Authority B, carrying a passport. An active, coordinated operation — distinct from ghost mode, which is a passive fallback.

Confusing transfer with ghost mode: transfer is a successful handoff to a new authority; ghost mode is what happens when no authority is reachable.

## Ghost Mode
_Avoid:_ offline mode, degraded mode, read-only

Read-only fallback when the authority is unreachable. The substrate remains visible (cached), but no interaction is possible — intents cannot be processed without an authority. Not a feature, a graceful degradation.

## Import Policy
_Avoid:_ validation, auth, permissions

Authority-defined rules for sanitizing incoming passports: filtering items, capping numeric fields, restricting roles. The authority's customs check at the border of a transfer.

## Connector
_Avoid:_ adapter, bridge, integration, bot

A platform adapter that presents an external service (Discord, Slack, Matrix) as an Interconnect room. The external service's data becomes navigable via the room/authority model.

## Transport
_Avoid:_ transfer (different concept), protocol, connection

The byte-moving abstraction: WebSocket, Unix socket, message queue. Transport is plumbing — how bytes move, not what they mean. Do not confuse with transfer (server-to-server authority handoff).
