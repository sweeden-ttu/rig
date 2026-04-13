//! **LM Studio** (OpenAI-compatible local server) — evaluate experiment outcomes and emit
//! **structured subagent instructions**.
//!
//! Uses **Chat Completions** (`.completions_api()`) for broad compatibility with LM Studio.
//!
//! ## Environment
//! ```text
//! export OPENAI_BASE_URL="http://localhost:1234/v1"
//! export OPENAI_API_KEY="lm-studio"
//! export LMSTUDIO_MODEL="your-loaded-model-id"
//! ```
//!
//! ## Input
//! - **First CLI argument**: path to a JSON file with experiment / benchmark outcomes, **or**
//! - **`EXPERIMENT_OUTCOMES_JSON`**: raw JSON string, **or**
//! - Built-in sample (Bell County–style summary).
//!
//! Markdown role briefs (orchestrator, evaluator, instruction writer) live at the repo root:
//! `agents/lmstudio/`.
//!
//! ## Run
//! ```text
//! cargo run -p rig-core --example lmstudio_experiment_subagent_orchestrator
//! cargo run -p rig-core --example lmstudio_experiment_subagent_orchestrator -- outcomes.json
//! ```

use rig::client::ProviderClient;
use rig::providers::openai;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum OverallVerdict {
    Pass,
    Partial,
    Fail,
    Inconclusive,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MetricCheck {
    name: String,
    passed: bool,
    evidence: String,
}

/// Structured outcome of the **evaluator** extractor (stage 1).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OutcomeAssessment {
    overall_verdict: OverallVerdict,
    evaluation_summary: String,
    metric_checks: Vec<MetricCheck>,
    blocking_issues: Vec<String>,
    risk_notes: Vec<String>,
    suggested_follow_up_arm: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct RoleDirective {
    subagent_role: String,
    markdown_instructions: String,
    success_criteria: String,
}

/// Briefs for downstream subagents (stage 2).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SubagentDirectives {
    briefs: Vec<RoleDirective>,
}

const SAMPLE_OUTCOMES: &str = r#"{
  "experiment_id": "bell_county_retrieval_sample",
  "hypotheses": [
    "H1: n_success reflects data_result_ok, not only HTTP 200",
    "H2: agent_build (cargo check) succeeds when RIG_ROOT is set"
  ],
  "artifacts": {
    "benchmark_json": "Researchanddevelopment/benchmarks/bell_county_retrieval_last.json",
    "agent_build": { "ok": true, "returncode": 0 }
  },
  "summary": {
    "n_success": 16,
    "n_http_ok": 17,
    "n_http_ok_but_no_data_signal": 1
  },
  "notes": "One path returned HTTP 200 but failed data heuristic (tiny page)."
}"#;

fn load_experiment_payload() -> anyhow::Result<String> {
    let mut args = std::env::args().skip(1);
    if let Some(path) = args.next() {
        return Ok(std::fs::read_to_string(path)?);
    }
    if let Ok(s) = std::env::var("EXPERIMENT_OUTCOMES_JSON") {
        if !s.is_empty() {
            return Ok(s);
        }
    }
    Ok(SAMPLE_OUTCOMES.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().pretty().init();

    let model = std::env::var("LMSTUDIO_MODEL")
        .or_else(|_| std::env::var("OPENAI_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let client = openai::CompletionsClient::from_env();

    let evaluator = client
        .extractor::<OutcomeAssessment>(model.clone())
        .preamble(
            "You evaluate experimental outcomes for rigor. Inputs describe benchmarks, \
             builds, and metrics. Do NOT treat HTTP 200 or non-empty HTML as success by itself; \
             prefer data_result_ok-style signals and agent build health when present. \
             Cite evidence from the JSON only. Output JSON matching the schema exactly.",
        )
        .build();

    let instructionist = client
        .extractor::<SubagentDirectives>(model)
        .preamble(
            "You write actionable markdown instruction briefs for specialized subagents \
             (retrieval, merge, judge, CI, audit). Each brief must list numbered steps and \
             clear success criteria. Output JSON matching the schema exactly.",
        )
        .build();

    let payload = load_experiment_payload()?;

    let assessment = evaluator.extract(&payload).await?;

    let directive_input = format!(
        "## Experiment payload\n{payload}\n\n## Evaluator JSON\n{}",
        serde_json::to_string_pretty(&assessment)?
    );

    let directives = instructionist.extract(&directive_input).await?;

    println!("=== Outcome assessment ===\n{}\n", serde_json::to_string_pretty(&assessment)?);
    println!("=== Subagent directives ===\n{}\n", serde_json::to_string_pretty(&directives)?);

    Ok(())
}
