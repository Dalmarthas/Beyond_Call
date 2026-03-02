# Codex Compatibility Notes

All skills from `skills/` were ported to `.agents/skills/` with Codex metadata (`agents/openai.yaml`).

## Hook caveat
Codex does not provide Claude-style hook automation (`PreToolUse`, `PostToolUse`, etc.).

For affected skills, this package now includes Codex-native alternatives:
- `continuous-learning-v2`: `instinct-cli.py observe` + `instinct-cli.py mine`
- `strategic-compact`: proactive boundary guidance + `suggest-compact.py` helper
- `security-scan`: Codex workspace scanning workflow (plus legacy-file checks when present)

Hook scripts remain optional for runtimes that support them.

For overlapping skill behavior, see `.codex/SKILL_PRECEDENCE.md`.
