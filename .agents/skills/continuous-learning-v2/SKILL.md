---
name: continuous-learning-v2
description: Instinct-based learning for Codex sessions that captures observations, mines repeated patterns, and evolves project-scoped instincts into reusable assets.
origin: ECC
version: 2.2.0
---

# Continuous Learning v2.2

Instinct-based learning for Codex workflows.

v2.2 keeps the same core model (observations -> instincts -> evolve/promote) but adds a Codex-native path that does not require runtime hooks.

## When to Activate

- You want to continuously learn from repeated coding workflows.
- You want project-specific instincts (to avoid cross-project contamination).
- You want to evolve learned patterns into skills/commands/agents.
- You want promotion from project scope to global scope once patterns are stable.

## Core Flow

1. Capture observations
2. Mine repeated patterns into instincts
3. Inspect and refine instincts
4. Evolve instincts into higher-level assets
5. Promote cross-project patterns to global scope

## Arbitration Role

`continuous-learning-v2` is a learning layer, not a phase driver.

- It can run alongside implementation and verification phases.
- It must not block `security-*`, `search-first`, `tdd-workflow`, or `verification-loop`.
- Prefer mining at checkpoints, not in the middle of active coding/debugging loops.

## Codex-Native Quick Start (No Hooks Required)

Use the CLI from repo root:

```bash
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py observe \
  --event checkpoint \
  --tool task-start \
  --input "Implementing auth middleware"

python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py observe \
  --event tool_start \
  --tool Edit \
  --input "src/auth/middleware.ts"

python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py observe \
  --event tool_complete \
  --tool Bash \
  --output "npm test -- auth passed"
```

Mine new instincts:

```bash
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py mine
```

Check status:

```bash
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py status
```

## Optional Hook Mode (If Runtime Supports Hooks)

If your runtime supports pre/post tool hooks, wire `hooks/observe.sh` into hook events.

The hook script writes observations to the same store used by the Codex-native path, so both modes are compatible.

## Commands

```bash
# Record one event (manual or hook payload)
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py observe [options]

# Mine repeated observation patterns into instincts
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py mine [options]

# View instincts and observation stats
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py status

# Export / import instincts
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py export --output instincts.yaml
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py import instincts.yaml --scope project

# Analyze evolution candidates (skills/commands/agents)
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py evolve [--generate]

# Promote project instincts to global scope
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py promote [instinct-id] [--dry-run]

# List known projects
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py projects
```

## Mining Rules (v2.2)

Default mined patterns include:
- `read-before-edit`
- `search-before-edit`
- `run-tests-after-edit`
- `review-diff-before-finish`

Minimum threshold is 3 occurrences (configurable with `--min-occurrences`).

## Scope Model

- `project` scope is default when inside a git project.
- `global` scope is used when no project is detected, or when explicitly requested.
- Promotion (`promote`) is used to move stable cross-project instincts to global.

## Storage

```text
~/.codex/homunculus/
  projects/<project-id>/
    observations.jsonl
    instincts/personal/
    instincts/inherited/
    evolved/{skills,commands,agents}/
  instincts/{personal,inherited}/
  evolved/{skills,commands,agents}/
  projects.json
```

## Background Observer

`agents/start-observer.sh` runs periodic mining in the background.

```bash
# Enable observer in config.json first
python3 .agents/skills/continuous-learning-v2/scripts/instinct-cli.py status
bash .agents/skills/continuous-learning-v2/agents/start-observer.sh start
bash .agents/skills/continuous-learning-v2/agents/start-observer.sh status
bash .agents/skills/continuous-learning-v2/agents/start-observer.sh stop
```

## Privacy

- All observations and instincts stay local by default.
- Export shares instincts (patterns), not raw conversation history.
- Project-scoped instincts are isolated per project.

## Related

- `.codex/commands/evolve.md`
- `.codex/commands/promote.md`
