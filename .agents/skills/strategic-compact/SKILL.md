---
name: strategic-compact
description: Suggest manual `/compact` at logical task boundaries to preserve quality during long Codex sessions.
origin: ECC
---

# Strategic Compact Skill

Suggest `/compact` at useful workflow boundaries, not arbitrary points.

## When to Activate

- Long sessions approaching context pressure
- Multi-phase tasks (research -> plan -> implementation -> validation)
- Major task switches inside one conversation
- After finishing a milestone and before starting unrelated work

## Why This Exists

Auto compaction (when available) can trigger mid-task and discard high-value active context.

Strategic compaction preserves quality by compacting only at phase boundaries.

## Codex-Native Behavior

When this skill is active, proactively suggest `/compact` when both are true:

1. A phase boundary is reached (e.g., planning complete, implementation complete, debugging finished)
2. Session context is materially large (many tool calls, large logs, broad file traversal)

Do **not** suggest compaction mid-implementation unless context quality is clearly degrading.

## Conflict Guardrails

- `strategic-compact` is advisory only; it must never force a phase switch.
- Do not suggest compaction during:
  - active red/green TDD cycles
  - active debugging on a reproducing failure
  - incomplete security/verification checkpoints
- Preferred timing: immediately after a completed milestone hand-off.

## Optional Counter Helper

Use the included helper to track tool checkpoints and prompt reminders:

```bash
# Track a checkpoint manually
python3 .agents/skills/strategic-compact/suggest-compact.py --tool Edit

# Read tool from hook payload (if runtime supports hooks)
python3 .agents/skills/strategic-compact/suggest-compact.py --from-hook

# Reset counters after compaction
python3 .agents/skills/strategic-compact/suggest-compact.py --reset
```

Defaults:
- First suggestion at 50 checkpoints
- Repeat every 25 checkpoints

Tune via `--threshold` and `--interval`.

## Decision Guide

| Transition | Compact? | Rationale |
|-----------|----------|-----------|
| Research -> Planning | Yes | Keep distilled plan, drop bulky exploration context |
| Planning -> Implementation | Yes | Plan is persistent; free context for execution |
| Implementation -> Testing | Maybe | Keep if tests depend on immediate code context |
| Debugging -> New feature | Yes | Remove dead-end traces and error noise |
| Mid-implementation | No | Active local state is still valuable |

## What Persists

After compaction, these remain available:
- `AGENTS.md` instructions
- Files on disk
- Git state
- Any explicitly written notes/todos

## Related

- `continuous-learning-v2` for pattern capture/evolution
