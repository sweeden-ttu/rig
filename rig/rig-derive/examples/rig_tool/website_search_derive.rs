//! Derived `#[rig_tool]` utilities for **public website text** research + LM Studio agent setup.
//!
//! Complements `rig-core/examples/website_search_lmstudio_agent.rs` with a minimal derive-only sample:
//! async fetch, sync name grep, and LM Studio / Rig builder hints.
//!
//! ## Run
//! ```text
//! export OPENAI_BASE_URL="http://localhost:1234/v1"
//! export OPENAI_API_KEY="lm-studio"
//! export LMSTUDIO_MODEL="unrestricted-knowledge-will-not-refuse-15b" or ""
//! cargo run -p rig-derive --example website_search_derive
//! ```

use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers;
use rig::tool::ToolError;
use rig_derive::rig_tool;
use reqwest::Url;

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(256 * 1024));
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[rig_tool(
    description = "GET http(s) URL; return tag-stripped text (max length). For county portals like https://bellcad.org/ .",
    params(url = "URL", max_chars = "Optional max chars (default 24000)"),
    required(url)
)]
async fn fetch_portal_page(url: String, max_chars: Option<u32>) -> Result<String, ToolError> {
    let u = Url::parse(&url).map_err(|e| ToolError::ToolCallError(format!("url: {e}").into()))?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err(ToolError::ToolCallError("http(s) only".into()));
    }
    let lim = max_chars.unwrap_or(24_000).max(400) as usize;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .user_agent("rig-derive-website_search_derive/1.0")
        .build()
        .map_err(|e| ToolError::ToolCallError(e.to_string().into()))?;
    let resp = client
        .get(u)
        .send()
        .await
        .map_err(|e| ToolError::ToolCallError(e.to_string().into()))?;
    if !resp.status().is_success() {
        return Err(ToolError::ToolCallError(format!("HTTP {}", resp.status()).into()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| ToolError::ToolCallError(e.to_string().into()))?;
    let t = strip_html_tags(&body);
    Ok(t.chars().take(lim).collect())
}

#[rig_tool(
    description = "Return lines containing `name` (case-insensitive).",
    params(haystack = "Text", name = "Name e.g. Tracy Myers"),
    required(haystack, name)
)]
fn grep_owner_name(haystack: String, name: String) -> Result<String, ToolError> {
    let n = name.to_lowercase();
    let mut v = Vec::new();
    for line in haystack.lines() {
        if line.to_lowercase().contains(&n) {
            v.push(line.trim().to_string());
        }
        if v.len() >= 30 {
            break;
        }
    }
    if v.is_empty() {
        Ok(format!("no matches for {name}"))
    } else {
        Ok(v.join("\n"))
    }
}

#[rig_tool(
    description = "LM Studio agent code builder: env vars, run commands for rig-core + rig-derive website examples, Bell CAD Tracy Myers seed (BELL_CAD_OWNER for other names)."
)]
fn lmstudio_code_builder_cheat_sheet() -> Result<String, ToolError> {
    Ok(
        "## LM Studio agent code builder (OpenAI-compatible local server)\n\
         \n\
         ```bash\n\
         export OPENAI_BASE_URL=http://localhost:1234/v1\n\
         export OPENAI_API_KEY=lm-studio\n\
         export LMSTUDIO_MODEL=<your-loaded-model-id>\n\
         # Optional: default owner for Bell CAD demos (override per run)\n\
         export BELL_CAD_OWNER='Tracy Myers'\n\
         ```\n\
         \n\
         ### rig-core (full agent + tools)\n\
         ```bash\n\
         cargo run -p rig-core --example website_search_lmstudio_agent --features derive\n\
         ```\n\
         \n\
         ### rig-derive (this example — minimal #[rig_tool] set)\n\
         ```bash\n\
         cargo run -p rig-derive --example website_search_derive\n\
         ```\n\
         \n\
         ### Bell CAD (https://bellcad.org/) — seed name\n\
         - Default: **Tracy Myers** (public portal research pattern).\n\
         - Other names: `BELL_CAD_OWNER='Jane Doe' cargo run ...`\n\
         \n\
         ### R&D tool list in rig-core example\n\
         Use tool `research_development_website_tool_catalog` there for fetch + grep + **lmstudio_rig_research_agent_builder** + seed.\n"
            .into(),
    )
}

#[rig_tool(
    description = "Bell CAD (bellcad.org) research seed; default Tracy Myers. Set BELL_CAD_OWNER for other names.",
    params(owner_name = "Owner to research; default Tracy Myers")
)]
fn bellcad_owner_research_seed(owner_name: Option<String>) -> Result<String, ToolError> {
    let owner = owner_name.unwrap_or_else(|| "Tracy Myers".to_string());
    Ok(format!(
        "## Bell County CAD seed — https://bellcad.org/\n\
         **Owner:** {owner}\n\
         \n\
         1. `fetch_portal_page` on https://bellcad.org/\n\
         2. `grep_owner_name` with name \"{owner}\"\n\
         3. Note: interactive Property Search may require a browser; this example is GET + text only.\n\
         \n\
         Generalize: same steps with a different owner string.\n",
        owner = owner
    ))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();

    let model = std::env::var("LMSTUDIO_MODEL")
        .or_else(|_| std::env::var("OPENAI_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let agent = providers::openai::Client::from_env()
        .completions_api()
        .agent(model)
        .preamble(
            "You are a research agent with website tools. Always call fetch_portal_page before summarizing a URL. \
             Use grep_owner_name on fetched text. \
             Use lmstudio_code_builder_cheat_sheet for LM Studio env + **agent code builder** run commands (rig-core and rig-derive examples). \
             Use bellcad_owner_research_seed for the Tracy Myers / Bell CAD workflow; other names via BELL_CAD_OWNER env.",
        )
        .max_tokens(2048)
        .tool(FetchPortalPage)
        .tool(GrepOwnerName)
        .tool(LmstudioCodeBuilderCheatSheet)
        .tool(BellcadOwnerResearchSeed)
        .build();

    let owner = std::env::var("BELL_CAD_OWNER").unwrap_or_else(|_| "Tracy Myers".to_string());
    let prompt = format!(
        "Fetch https://bellcad.org/ and check whether \"{owner}\" appears in the visible text. Summarize."
    );

    println!("User: {prompt}");
    match agent.prompt(prompt).max_turns(20).await {
        Ok(r) => println!("Agent: {r}"),
        Err(e) => eprintln!("error: {e}"),
    }
}
