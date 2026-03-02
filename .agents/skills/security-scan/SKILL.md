---
name: security-scan
description: Scan Codex workspace configuration and agent assets for security misconfigurations, injection risks, and secret exposure.
origin: ECC
---

# Security Scan Skill

Audit Codex configuration and agent assets for security issues.

## When to Activate

- After editing `AGENTS.md` or `.codex/config.toml`
- After adding/updating `.codex/agents/`, `.codex/commands/`, `.codex/contexts/`, `.codex/rules/`
- Before committing/pushing infra or automation changes
- When onboarding into a repository that already has Codex assets
- As a periodic hygiene check

## When Not to Activate

- Routine feature coding with no config/policy/runtime-asset changes
- As a substitute for code-level security design review (use `security-review`)

## Conflict Guardrails

- If both `security-scan` and `verification-loop` apply, run both at checkpoint time:
  - `security-scan` for policy/config/assets risk
  - `verification-loop` for build/test/readiness
- If both `security-scan` and `security-review` apply:
  - `security-review` drives code-level changes
  - `security-scan` gates repository/config safety before merge/push

## What It Scans

| Path | Checks |
|------|--------|
| `AGENTS.md`, `.codex/AGENTS.md` | Prompt-injection vectors, unsafe auto-run instructions, secret leakage |
| `.codex/config.toml` | Over-permissive server/tool config, unsafe command defaults |
| `.codex/agents/`, `.codex/commands/`, `.codex/contexts/`, `.codex/rules/` | Command injection patterns, risky shell examples, missing safety constraints |
| `.agents/skills/` | Unsafe workflow instructions, secret literals, high-risk copy/paste commands |
| Legacy config files (if present) | `settings.json`, `mcp.json`, hook scripts, runtime-specific escape hatches |

## Primary Scanner (AgentShield)

```bash
# Check availability
npx ecc-agentshield --version

# Run workspace scan
npx ecc-agentshield scan --path .

# CI-friendly JSON
npx ecc-agentshield scan --path . --format json --min-severity medium

# Safe auto-fixes only
npx ecc-agentshield scan --path . --fix
```

## Codex-Focused Manual Checks

Run these even if AgentShield is unavailable:

```bash
# Potential secrets
rg -n "(sk-[A-Za-z0-9]{10,}|api[_-]?key|secret|token|password)" AGENTS.md .codex .agents --hidden

# Dangerous shell patterns in docs/scripts
rg -n "curl\s+.*\|\s*(bash|sh)|sudo\s+|chmod\s+777|rm\s+-rf\s+/" .codex .agents --hidden

# Prompt-injection style directives
rg -n "ignore previous|bypass|disable security|run without confirmation" AGENTS.md .codex .agents --hidden

# Review modified files before push
git diff -- . ':!node_modules'
```

## Severity Guidance

- Critical: hardcoded credentials, unrestricted remote shell execution
- High: broad trust or bypass directives in agent/command docs
- Medium: unsafe examples without guardrails, missing validation steps
- Low/Info: clarity and hardening opportunities

## Remediation Workflow

1. Fix critical/high findings first.
2. Re-run scan until no critical/high remain.
3. Document accepted medium/low risk in commit notes.
4. Re-check with `git diff` before commit.

## CI Example

```yaml
- name: AgentShield
  run: npx ecc-agentshield scan --path . --min-severity medium --format json
```

## Links

- [AgentShield GitHub](https://github.com/affaan-m/agentshield)
- [AgentShield npm](https://www.npmjs.com/package/ecc-agentshield)
