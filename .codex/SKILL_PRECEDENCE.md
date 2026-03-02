# Skill Precedence Policy (Codex)

This policy resolves overlapping skills without disabling implicit invocation.

## Core Rule

At any moment, exactly one skill is the **driving workflow**. Other triggered skills are **supporting** and must not interrupt the driver.

## Priority Order (Highest -> Lowest)

1. **Security gate**
   - `security-scan`: workspace/config/agent asset security
   - `security-review`: code-level security for auth/input/secrets/payments
2. **Discovery gate**
   - `search-first`: evaluate existing solutions before building net-new
3. **Implementation gate**
   - `tdd-workflow`: red/green/refactor for behavior changes
4. **Readiness gate**
   - `verification-loop`: build/lint/typecheck/tests/security before PR/push
5. **Context hygiene**
   - `strategic-compact`: suggest `/compact` only at phase boundaries
6. **Learning layer (non-blocking)**
   - `continuous-learning-v2`: capture/mine patterns; never override driver

## Conflict Recipes

### `search-first` vs `tdd-workflow`
- Run `search-first` first when architecture/dependency choice is unresolved.
- Once approach is chosen, `tdd-workflow` becomes driver.

### `tdd-workflow` vs `verification-loop`
- During red/green cycles, `tdd-workflow` is driver.
- `verification-loop` runs only at milestone/hand-off boundaries, not after each micro-edit.

### `security-scan` vs `verification-loop`
- If config/infrastructure/docs changed: run both.
- `security-scan` checks policy/config risk; `verification-loop` checks build/test readiness.

### `strategic-compact` vs active work
- Never compact mid red/green cycle, active debugging, or in-progress refactor chunk.
- Suggest compaction only after checkpoint completion.

### `continuous-learning-v2` vs all others
- Observation/mining is passive and asynchronous.
- Never block implementation/verification waiting on mining.

## Operational Defaults

- If two skills conflict, choose the higher-priority driver.
- Supporting skills may add checks, but cannot force phase switches.
- When in doubt, continue current phase and defer lower-priority skill to the next checkpoint.
