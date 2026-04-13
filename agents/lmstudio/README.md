# LM Studio agents (local inference)

These files describe **roles** used by the Rig example
`rig-core/examples/lmstudio_experiment_subagent_orchestrator.rs`. Point **LM Studio**
OpenAI-compatible server at `http://localhost:1234/v1` and load a model, then run the
binary with benchmark / experiment JSON.

## Environment

```text
export OPENAI_BASE_URL="http://localhost:1234/v1"
export OPENAI_API_KEY="lm-studio"
export LMSTUDIO_MODEL="your-model-id-from-lm-studio"
```

## Artifacts

| File | Role |
|------|------|
| [experiment-evaluator.md](experiment-evaluator.md) | Score experimental outcomes vs. hypotheses and endpoints (not raw HTTP 200). |
| [subagent-instruction-writer.md](subagent-instruction-writer.md) | Turn evaluation + context into actionable briefs for subagents. |
| [orchestrator.md](orchestrator.md) | When to spawn specialists, merge results, and stop. |

## Runnable harness

```text
cd rig && cargo run -p rig-core --example lmstudio_experiment_subagent_orchestrator -- /path/to/outcomes.json
```

If no path is given, the example uses stdin or the `EXPERIMENT_OUTCOMES_JSON` environment variable, else a small built-in sample.
