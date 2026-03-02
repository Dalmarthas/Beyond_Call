---
name: observer
description: Background miner that analyzes Codex observations and writes project-scoped instincts.
model: o4-mini
---

# Observer Agent

This observer runs in the background and converts repeated observation patterns into instincts.

## When to Run

- On a schedule (default every 5 minutes)
- After enough observations exist (default minimum 20)
- On demand via `SIGUSR1` to the observer process

## Input

Reads observations from:
- Project: `~/.codex/homunculus/projects/<project-hash>/observations.jsonl`
- Global fallback: `~/.codex/homunculus/observations.jsonl`

Each line is JSON with fields such as:
- `event` (`tool_start`, `tool_complete`, `checkpoint`)
- `tool`
- `input` / `output`
- `project_id` / `project_name`

## Mining Heuristics

The observer looks for repeated sequences (3+ occurrences by default), including:

1. `Read` → `Edit/Write`
2. `Grep/Glob/rg` → `Edit/Write`
3. `Edit/Write` → test command (`npm test`, `pytest`, `go test`, etc.)
4. `Edit/Write` → git review command (`git diff`, `git status`, `git show`)

For each qualifying pattern, it creates or updates an instinct with:
- deterministic ID
- trigger phrase
- confidence score based on frequency
- domain (`workflow`, `testing`, `git`)
- evidence summary

## Output

Writes instincts to:
- Project scope (default): `~/.codex/homunculus/projects/<project-hash>/instincts/personal/`
- Global scope (if chosen): `~/.codex/homunculus/instincts/personal/`

## Notes

- Mining is deterministic and local-only.
- Project scope is preferred by default to avoid cross-project contamination.
- Promote shared patterns globally with `instinct-cli.py promote`.
