---
name: lmstudio-orchestrator
description: >-
  High-level loop for local LM Studio: evaluate outcomes, dispatch subagent briefs, and
  decide whether to continue, branch, or stop. Use with the Rig orchestrator example or
  as a Cursor agent preamble.
---

## Loop

1. **Ingest** experiment artifacts (benchmark JSON, logs, hashes).
2. **Evaluate** — run evaluator role; require explicit verdict on primary endpoints.
3. **Plan** — instruction writer emits briefs per subagent role (parallelizable when independent).
4. **Gate** — if blocking issues remain, either fix inputs or stop; do not spin subagents without a hypothesis.
5. **Record** — append run id, model id, and LM Studio base URL (non-secret) for reproducibility.

## Stop conditions

- Verdict **pass** on primary endpoints with no blocking issues.
- Budget or round cap reached (set externally).
- **Inconclusive** after N repeats — document variance before adding factors.

## Delegation

Subagents receive **only** their brief + shared context links; avoid dumping full logs unless
the brief’s success criteria require them.
