---
layout: home

hero:
  name: "Interconnect"
  text: "Federation Protocol"
  tagline: "Authoritative handoff for connected rooms."
  actions:
    - theme: brand
      text: Get Started
      link: /introduction
    - theme: alt
      text: Protocol Reference
      link: /protocol

features:
  - title: Single Authority
    details: Each room is owned by one authority. No state merging, no split-brain attacks.
  - title: Intent-Based
    details: Clients send intent, authorities compute results. Never trust client state.
  - title: Graceful Degradation
    details: When authorities go offline, static room data remains. Ghost mode, not void.
  - title: Daemon + CLI
    details: A persistent daemon holds room connections. Short-lived CLI commands and AI assistant hooks talk to it over a Unix socket.
    link: /daemon
  - title: Agent Orchestration
    details: Wire Claude Code, Gemini CLI, or any AI assistant into rooms using lifecycle hooks. Messages in, responses out — routing is outside the assistant's concern.
    link: /agent-orchestration
---
