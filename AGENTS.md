# Codex Package — AGENTS

This branch is a Codex-only package.

## Primary Paths

- Skills: `.agents/skills/`
- Codex config and references: `.codex/`
- Skill conflict policy: `.codex/SKILL_PRECEDENCE.md`
- Transfer details: `.codex/TRANSFER_REPORT.md`
- Compatibility caveats: `.agents/skills/CODEX_COMPATIBILITY_NOTES.md`

## How to Use

1. Use `SKILL.md` files under `.agents/skills/` as the primary skill source.
2. Use `.codex/commands/`, `.codex/agents/`, `.codex/contexts/`, and `.codex/rules/` as reusable playbooks.
3. Prefer `.codex/config.toml` conventions when setting local Codex config.

## Notes

- This repo intentionally excludes Claude/Cursor/OpenCode runtime assets.
- Some skills still describe hook-driven flows; in Codex these are guidance-first workflows, not guaranteed automatic hooks.
