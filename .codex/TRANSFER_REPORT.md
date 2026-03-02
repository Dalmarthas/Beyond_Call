# Codex Transfer Report

This report documents content transferred from the original ECC folders into Codex-usable structure.

## Skills
- Source: `skills/`
- Target: `.agents/skills/`
- Action: Copied missing skills so `.agents/skills/` now contains all skill directories from `skills/`.
- Codex metadata: Added `agents/openai.yaml` for each newly copied skill.

## Agents
- Source: `agents/*.md`
- Target: `.codex/agents/`
- Action: Transferred Codex-applicable agent prompt docs.
- Skipped: `chief-of-staff.md` (depends on hook-enforced and tool-specific workflows unavailable in Codex).

## Commands
- Source: `commands/*.md`
- Target: `.codex/commands/`
- Action: Transferred Codex-applicable command playbooks.
- Skipped (Claude runtime dependent): `claw.md`, `instinct-export.md`, `instinct-import.md`, `instinct-status.md`, `learn.md`, `learn-eval.md`, `multi-backend.md`, `multi-execute.md`, `multi-frontend.md`, `multi-plan.md`, `multi-workflow.md`, `pm2.md`, `projects.md`, `sessions.md`, `setup-pm.md`.

## Contexts
- Source: `contexts/*.md`
- Target: `.codex/contexts/`
- Action: Transferred all context docs.

## Examples
- Source: `examples/*`
- Target: `.codex/examples/`
- Action: Transferred and renamed `*CLAUDE.md` examples to `*AGENTS.md` naming for Codex.

## Rules
- Source: `rules/**`
- Target: `.codex/rules/**`
- Action: Transferred non-hook rule files.
- Skipped as non-applicable to Codex hook model: `rules/*/hooks.md`.

## Scripts
- Source: `scripts/**`
- Target: `.codex/scripts/**`
- Action: Transferred generic validator scripts and codemap generator.
- Skipped: Claude runtime and hook/session-specific scripts.

## Tests
- Source: `tests/**`
- Target: `.codex/tests/**`
- Action: Transferred CI validator test suite (`tests/ci/validators.test.js`) as most relevant to transferred validator scripts.
- Skipped: Hook/session-manager/package-manager/claw tests tied to Claude runtime paths and hook system.
