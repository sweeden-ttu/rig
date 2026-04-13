//! Website search tools + **LM Studio** (OpenAI-compatible) agent for county public-portal research.
//!
//! ## Seed use case
//! Research **[Bell County Tax Appraisal District](https://bellcad.org/)** (property search / owner names).
//! Default owner in the prompt is **Tracy Myers**; set `BELL_CAD_OWNER` to generalize.
//!
//! ## LM Studio / OpenAI-compatible env
//! ```text
//! export OPENAI_BASE_URL="http://localhost:1234/v1"
//! export OPENAI_API_KEY="lm-studio"   # LM Studio accepts any non-empty key
//! export LMSTUDIO_MODEL="your-loaded-model-id"
//! ```
//!
//! Uses **Chat Completions** (`.completions_api()`) for broad local-server compatibility.
//!
//! ## Tools (research + R&D)
//! - `fetch_public_webpage` — GET + strip tags  
//! - `find_name_mentions` — grep lines for a person name  
//! - `extract_hrefs_from_html` — pull `href` URLs from HTML (optional keyword filter) for **Property Search** discovery  
//! - `bellcad_public_entrypoints` — seed URLs + workflow notes for [Bell CAD](https://bellcad.org/)  
//! - `lmstudio_rig_research_agent_builder` — LM Studio + Rig agent / `#[rig_tool]` boilerplate  
//! - `bellcad_owner_research_seed` — derive-friendly seed prompt; default **Tracy Myers**, override with `BELL_CAD_OWNER`
//!
//! Run:
//! ```text
//! cargo run -p rig-core --example website_search_lmstudio_agent --features derive
//! ```

use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::openai;
use rig::tool::ToolError;
use rig_derive::rig_tool;
use reqwest::Url;
use url::Url as UrlJoin;

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(512 * 1024));
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
    description = "GET an http(s) URL and return body truncated to max_chars. Default strip_html=true (readable text). Set strip_html=false to keep raw HTML for extract_hrefs_from_html.",
    params(
        url = "Absolute URL (http or https)",
        max_chars = "Max characters to return (default 32000, min 512)",
        strip_html = "If true (default), strip tags and normalize whitespace; if false, return raw HTML substring"
    ),
    required(url)
)]
async fn fetch_public_webpage(
    url: String,
    max_chars: Option<u32>,
    strip_html: Option<bool>,
) -> Result<String, ToolError> {
    let parsed = Url::parse(&url).map_err(|e| ToolError::ToolCallError(format!("invalid url: {e}")))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(ToolError::ToolCallError("only http/https allowed".into()));
    }
    let limit = max_chars.unwrap_or(32_000).max(512) as usize;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(concat!(
            "rig-website_search_lmstudio_agent/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/0xPlaygrounds/rig)"
        ))
        .build()
        .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

    let resp = client
        .get(parsed)
        .send()
        .await
        .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(ToolError::ToolCallError(format!("HTTP {}", resp.status())));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| ToolError::ToolCallError(e.to_string()))?;
    let processed = if strip_html.unwrap_or(true) {
        strip_html_tags(&body)
    } else {
        body
    };
    let mut out: String = processed.chars().take(limit).collect();
    if processed.chars().count() > limit {
        out.push_str("\n... [truncated]");
    }
    Ok(out)
}

/// Collect `href` targets from HTML; optionally keep only those containing `keyword` (case-insensitive).
fn collect_hrefs(html: &str, keyword: Option<&str>, base: Option<&str>) -> Vec<String> {
    let kw = keyword.map(|k| k.to_lowercase());
    let base_url = base.and_then(|b| UrlJoin::parse(b).ok());
    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::new();

    for part in html.split("href=\"").skip(1) {
        if out.len() >= 80 {
            break;
        }
        push_href_part(part, '"', &kw, &base_url, &mut seen, &mut out);
    }
    for part in html.split("href='").skip(1) {
        if out.len() >= 80 {
            break;
        }
        push_href_part(part, '\'', &kw, &base_url, &mut seen, &mut out);
    }
    out
}

fn push_href_part(
    chunk: &str,
    end_quote: char,
    kw: &Option<String>,
    base_url: &Option<UrlJoin>,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<String>,
) {
    if out.len() >= 80 {
        return;
    }
    let Some(end) = chunk.find(end_quote) else {
        return;
    };
    let raw = chunk[..end].trim();
    if raw.is_empty() || raw.starts_with('#') || raw.to_lowercase().starts_with("javascript:") {
        return;
    }
    let resolved = if let Some(b) = base_url {
        b.join(raw).map(|u| u.to_string()).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    };
    if let Some(ref k) = kw {
        if !resolved.to_lowercase().contains(k) {
            return;
        }
    }
    if seen.insert(resolved.clone()) {
        out.push(resolved);
    }
}

#[rig_tool(
    description = "Extract href URLs from HTML (e.g. after fetch_public_webpage). Optional keyword filter (e.g. property, search, map). Optional base_url resolves relative links for https://bellcad.org/ .",
    params(
        html_text = "Raw HTML",
        keyword_substring = "If set, only hrefs containing this substring (case-insensitive)",
        base_url = "e.g. https://bellcad.org/ for relative paths"
    ),
    required(html_text)
)]
fn extract_hrefs_from_html(
    html_text: String,
    keyword_substring: Option<String>,
    base_url: Option<String>,
) -> Result<String, ToolError> {
    let hrefs = collect_hrefs(
        &html_text,
        keyword_substring.as_deref(),
        base_url.as_deref(),
    );
    if hrefs.is_empty() {
        Ok("No matching hrefs.".into())
    } else {
        Ok(hrefs.join("\n"))
    }
}

#[rig_tool(
    description = "Official Bell County CAD (https://bellcad.org/) entry points and workflow: Property Search, Tracy Myers seed name (override BELL_CAD_OWNER), and JS-heavy search caveats."
)]
fn bellcad_public_entrypoints() -> Result<String, ToolError> {
    Ok(r#"## Bell County Tax Appraisal District — public portal

**Site:** https://bellcad.org/ ([Bell CAD](https://bellcad.org/))

### High-value UI areas (from public home page)
- **Property Search** — primary owner / parcel lookup (often JS-driven after navigation).
- **Interactive map** — parcel context.
- **Online forms** — exemptions, renditions.

### Seed name (generalize)
- Default research subject: **Tracy Myers**
- Environment: `BELL_CAD_OWNER` to run the same tool flow for other owners.

### Recommended agent steps
1. `fetch_public_webpage` on `https://bellcad.org/`
2. `extract_hrefs_from_html` with `keyword_substring` = `property` or `search` and `base_url` = `https://bellcad.org/`
3. Optionally fetch 1–2 discovered URLs and `find_name_mentions` with the target name
4. If the name only appears after interactive search, state that limitation clearly.
"#
    .to_string())
}

#[rig_tool(
    description = "Find lines in a text blob that mention a name (case-insensitive). Returns up to 40 matching lines with context.",
    params(
        full_text = "Plain text or stripped HTML text to search",
        name = "Person or entity name, e.g. Tracy Myers"
    ),
    required(full_text, name)
)]
fn find_name_mentions(full_text: String, name: String) -> Result<String, ToolError> {
    let needle = name.to_lowercase();
    let lines: Vec<&str> = full_text.lines().collect();
    let mut hits: Vec<String> = Vec::new();
    for line in lines {
        if line.to_lowercase().contains(&needle) {
            let t = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if !t.is_empty() {
                hits.push(t);
            }
        }
        if hits.len() >= 40 {
            break;
        }
    }
    if hits.is_empty() {
        Ok(format!("No lines contain \"{name}\"."))
    } else {
        Ok(hits.join("\n"))
    }
}

#[rig_tool(
    description = "Return boilerplate for building Rig agents against LM Studio (OpenAI-compatible local server): env vars, Chat Completions switch, and #[rig_tool] research pattern."
)]
fn lmstudio_rig_research_agent_builder() -> Result<String, ToolError> {
    Ok(r#"## LM Studio + Rig — research agent builder (OpenAI-compatible)

### Environment
- `OPENAI_BASE_URL` — e.g. `http://localhost:1234/v1`
- `OPENAI_API_KEY` — any non-empty string (LM Studio)
- `LMSTUDIO_MODEL` — model id as shown in LM Studio (fallback: `OPENAI_MODEL`)

### Rust client (Chat Completions)
```rust
use rig::client::{CompletionClient, ProviderClient};
use rig::providers::openai;

let model = std::env::var("LMSTUDIO_MODEL")
    .or_else(|_| std::env::var("OPENAI_MODEL"))
    .unwrap_or_else(|_| "gpt-4o-mini".into());

let agent = openai::Client::from_env()
    .completions_api()
    .agent(model)
    .preamble("You are a research agent. Use tools for live web text; do not invent search results.")
    .tool(YourFetchTool)
    .tool(YourNameGrepTool)
    .tool(YourHrefTool)
    .build();
```

### Tool catalog (this repo’s `website_search_lmstudio_agent` example)
1. `fetch_public_webpage` — GET + strip tags.
2. `find_name_mentions` — grep fetched text for a person name.
3. `extract_hrefs_from_html` — discover Property Search / map links from HTML.
4. `bellcad_public_entrypoints` — Bell CAD workflow + [bellcad.org](https://bellcad.org/) seed.
5. `bellcad_owner_research_seed` — copy-paste task template (Tracy Myers → any name).
6. `lmstudio_rig_research_agent_builder` — this cheat sheet.
7. Optional: Playwright (separate project) for JS-only search forms.

### Bell CAD seed
- Portal: https://bellcad.org/ — Property Search / owner research.
- Default seed name: **Tracy Myers**; generalize via `BELL_CAD_OWNER`.
"#
    .to_string())
}

#[rig_tool(
    description = "Seed prompt text for Bell CAD (bellcad.org) owner/property search; default owner Tracy Myers.",
    params(owner_name = "Owner name to research; omit for Tracy Myers")
)]
fn bellcad_owner_research_seed(owner_name: Option<String>) -> Result<String, ToolError> {
    let owner = owner_name.unwrap_or_else(|| "Tracy Myers".to_string());
    Ok(format!(
        r#"## Bell County CAD research seed (https://bellcad.org/)

**Owner focus:** {owner}

### Suggested tool flow
1. `fetch_public_webpage` with `strip_html=false` on `https://bellcad.org/` to preserve `href` attributes.
2. `extract_hrefs_from_html` with keyword `search` or `property` and `base_url` `https://bellcad.org/`.
3. `fetch_public_webpage` with default stripping on the same URL (or a discovered search URL) for readable text.
4. `find_name_mentions` on stripped text with name "{owner}".
5. Summarize: navigation hints, whether the name appears in static copy, and what requires interactive Property Search (JS).

### Generalization
Replace the owner string to research other individuals using the same tool chain.
"#,
        owner = owner
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().pretty().init();

    let model = std::env::var("LMSTUDIO_MODEL")
        .or_else(|_| std::env::var("OPENAI_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let agent = openai::Client::from_env()
        .completions_api()
        .agent(model)
        .preamble(
            "You are an R&D assistant for U.S. county public web portals (property tax / appraisal). \
             You MUST use tools: fetch pages before claiming what a site contains; \
             use extract_hrefs_from_html after a fetch to find Property Search / map URLs on portals like https://bellcad.org/ ; \
             use find_name_mentions on fetched text to check for a person name; \
             use bellcad_public_entrypoints for the Bell CAD seed workflow; \
             use lmstudio_rig_research_agent_builder when the user asks how to build such agents in Rust+Rig+LM Studio; \
             use bellcad_owner_research_seed when the user wants the Bell CAD (bellcad.org) workflow template. \
             Never fabricate search results.",
        )
        .max_tokens(2048)
        .tool(FetchPublicWebpage)
        .tool(FindNameMentions)
        .tool(ExtractHrefsFromHtml)
        .tool(BellcadPublicEntrypoints)
        .tool(LmstudioRigResearchAgentBuilder)
        .tool(BellcadOwnerResearchSeed)
        .build();

    let owner = std::env::var("BELL_CAD_OWNER").unwrap_or_else(|_| "Tracy Myers".to_string());
    let prompt = format!(
        "Using your tools: (1) fetch https://bellcad.org/ with strip_html=false, (2) extract_hrefs_from_html with keyword \"search\" or \"property\" (base_url https://bellcad.org/), \
         (3) fetch again with default stripping and find_name_mentions for \"{owner}\", \
         (4) summarize hrefs found, whether the name appears in static text, and if interactive Property Search is still required."
    );

    println!("User:\n{prompt}\n");
    let response = agent.prompt(prompt).max_turns(24).await?;
    println!("Agent:\n{response}");

    Ok(())
}
