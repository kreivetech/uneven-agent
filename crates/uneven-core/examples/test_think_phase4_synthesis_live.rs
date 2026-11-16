//! Standalone test binary to execute a single live Gemini LLM call
//! testing Turn 4: Synthesis & Final Assured Patch (`think_synthesis`)
//! of uneven-core's `think` reasoning engine.
//!
//! Reconciles Phase 1 Draft, Turn 2 Critic Findings, and Turn 3 Integrity Verdict
//! into the final human-facing proposal and patches with max_tokens = 4000.
//!
//! Uses ONLY `GOOGLE_API_KEY` (or `GOOGLE_API_KEY_FREE` / `GEMINI_API_KEY`) from `.env`.
//! No parsing is performed — purely inspecting the raw AI final synthesis output.
//!
//! Run with:
//!   cargo run -p uneven-core --example test_think_phase4_synthesis_live

use std::path::PathBuf;
use std::time::Instant;
use llmcraft::{CompletionParams, LlmClient, Provider};
use uneven_core::infrastructure::security::laws;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load environment variables (.env from current or parent workspace directory)
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_path(PathBuf::from("../../.env"));
    }

    println!("============================================================");
    println!("  Uneven Core — Live Think Turn 4 (Final Synthesis) Benchmark");
    println!("============================================================");

    let api_key = std::env::var("GOOGLE_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_API_KEY_FREE"))
        .or_else(|_| std::env::var("GEMINI_API_KEY"))
        .expect("No Google API Key found in .env (checked GOOGLE_API_KEY, GOOGLE_API_KEY_FREE, GEMINI_API_KEY)");

    let masked_key = if api_key.len() > 8 {
        format!("{}...{}", &api_key[..4], &api_key[api_key.len() - 4..])
    } else {
        "***".to_string()
    };
    println!("🔑 Provider API Key detected: {masked_key}");

    // 2. Load Turn 4 System Rules (Final Synthesis & Patch Delivery)
    let synthesis_base = laws::get_context("think_synthesis");
    let synthesis_system = format!(
        "{}\n\n[PHASE: FINAL SYNTHESIS - Eliminate all issues flagged by the Critic and enforce Scope & Code Integrity. Deliver the final proposal and structured Markdown patches.]",
        synthesis_base
    );
    println!("📜 System Rules loaded from `laws::get_context(\"think_synthesis\")`");

    // 3. Load Prior Phases from disk
    let draft_dump_path = PathBuf::from("target/reports/test_think_draft_raw_response.md");
    let draft_content = if draft_dump_path.exists() {
        let raw = std::fs::read_to_string(&draft_dump_path)?;
        raw.split_once("## Raw LLM Output Content\n\n")
            .map(|(_, c)| c.trim().to_string())
            .unwrap_or(raw)
    } else {
        "Preliminary draft proposing DbConfig and PgPoolOptions with backoff.".to_string()
    };
    println!("📥 Phase 1 Draft loaded ({} bytes)", draft_content.len());

    let critic_dump_path = PathBuf::from("target/reports/test_think_critic_raw_response.md");
    let critic_content = if critic_dump_path.exists() {
        let raw = std::fs::read_to_string(&critic_dump_path)?;
        raw.split_once("## Raw Critic Audit Output Content\n\n")
            .map(|(_, c)| c.trim().to_string())
            .unwrap_or(raw)
    } else {
        "Critic findings recommending backoff cap and env var validation.".to_string()
    };
    println!("📥 Turn 2 Critic Findings loaded ({} bytes)", critic_content.len());

    let integrity_dump_path = PathBuf::from("target/reports/test_think_integrity_raw_response.md");
    let integrity_content = if integrity_dump_path.exists() {
        let raw = std::fs::read_to_string(&integrity_dump_path)?;
        raw.split_once("## Raw Integrity Guard Output Content\n\n")
            .map(|(_, c)| c.trim().to_string())
            .unwrap_or(raw)
    } else {
        "VERDICT: ALIGNED".to_string()
    };
    println!("📥 Turn 3 Integrity Verdict loaded: {}", integrity_content.lines().next().unwrap_or("ALIGNED"));

    // 4. Codebase Context & User Task
    let task = "Analyze the authentication gateway's database connection pooling in `src/infrastructure/db.rs` and `src/services/auth.rs` for leaks or undefined limits. Propose a refactoring to implement secure asynchronous pooling with exponential backoff and monitoring metrics to address PostgreSQL connection exhaustion under high concurrent load.";

    let context_block = r#"## RELEVANT CONTEXT & CODEBASE MAP

### File: src/infrastructure/db.rs
```rust
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

pub struct DbConnection {
    pub pool: PgPool,
}

impl DbConnection {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }
}
```

### File: src/services/auth.rs
```rust
use crate::infrastructure::db::DbConnection;

pub struct AuthService {
    db: DbConnection,
}

impl AuthService {
    pub async fn verify_token(&self, token: &str) -> Result<bool, String> {
        let row = sqlx::query("SELECT id FROM active_sessions WHERE token = $1")
            .bind(token)
            .fetch_optional(&self.db.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.is_some())
    }
}
```
"#;

    let synthesis_prompt = format!(
        "### User Request:\n{}\n\n### Context:\n{}\n\n### Initial Draft:\n{}\n\n### Internal Critic Audit Findings:\n{}\n\n### Scope & Code Integrity Directives:\n{}\n\n\
        Produce the FINAL, flawless response for the user in clean Markdown:\n\
        1. Human-Facing Overview & Explanations: Formulate naturally in the user's primary language without robotic or bureaucratic phrasing.\n\
        2. Technical & Code Discipline: All source code, diff hunks, functions, variable names, and comments MUST strictly remain in English.\n\
        3. Reconciled Code Implementation: Integrate the Critic's refinements (add a MAX_RETRY_DELAY cap to backoff, enforce DATABASE_URL from environment) into the finalized surgical Markdown patches (`### File: <path>`, `**Action**: edit`).\n\
        4. Terminal Commands: Emit [EXEC_CMD: <command>] for any compilation or cargo checks recommended.",
        task, context_block, draft_content, critic_content, integrity_content
    );

    // 5. Configure LLM Client with 10k Output Token Ceiling
    let model_name = "gemini-2.5-flash";
    let client = LlmClient::builder(Provider::Gemini)
        .model(model_name)
        .api_key(&api_key)
        .build()?;

    let max_tokens = 10000;
    let params = CompletionParams::new(&synthesis_prompt)
        .with_system(&synthesis_system)
        .with_temperature(0.2)
        .with_max_tokens(max_tokens);

    println!("\n⚙️  Configuração da Chamada:");
    println!("   - Modelo:       {}", model_name);
    println!("   - Temperatura:  0.2 (temperatura oficial da Fase Final - Synthesis)");
    println!("   - Max Tokens:   {} (teto expandido para 10k para plano completo e diffs irrestritos)", max_tokens);

    println!("\n🚀 Enviando 1 chamada controlada para o Google Gemini...");
    let start = Instant::now();
    let response = client.complete_with_params(params).await?;
    let duration = start.elapsed();

    println!("✅ Resposta Final da Síntese recebida em {:.2?}!", duration);

    // 6. Telemetria
    if let Some(usage) = &response.usage {
        println!("\n📊 Telemetria de Tokens:");
        println!("   - Tokens de Entrada (Toda a Cadeia de Raciocínio): {}", usage.prompt_tokens);
        println!("   - Tokens de Saída (Síntese & Patches Finais):       {}", usage.completion_tokens);
        println!("   - Total de Tokens:                                 {}", usage.total_tokens);
    }
    if let Some(cost) = response.cost_usd {
        println!("   - Custo Estimado:                                  ${:.6} USD", cost);
    }

    // 7. Exibir Resposta Bruta da Síntese Final (Sem Parser)
    println!("\n------------------------------------------------------------");
    println!("💬 RESPOSTA BRUTA DA SÍNTESE FINAL (TURN 4 - THINK SYNTHESIS):\n");
    println!("{}", response.content.trim());
    println!("------------------------------------------------------------");

    // 8. Salvar Dump da Síntese Final em Disco
    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let raw_path = output_dir.join("test_think_synthesis_raw_response.md");

    let dump_content = format!(
        "# Gemini Raw Think Turn 4 (Final Synthesis) Response Dump\n\n\
        - **Timestamp:** {}\n\
        - **Model:** {}\n\
        - **Temperature:** 0.2\n\
        - **Max Tokens:** {}\n\
        - **Latency:** {:.2?}\n\
        - **Prompt Tokens:** {}\n\
        - **Completion Tokens:** {}\n\
        - **Total Tokens:** {}\n\
        - **Cost USD:** ${:.6}\n\n\
        ---\n\n\
        ## Raw Final Synthesis Output Content\n\n\
        {}\n",
        chrono::Local::now().to_rfc3339(),
        model_name,
        max_tokens,
        duration,
        response.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
        response.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
        response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
        response.cost_usd.unwrap_or(0.0),
        response.content.trim(),
    );

    std::fs::write(&raw_path, &dump_content)?;
    println!("💾 Resposta bruta da síntese final salva em: {}", raw_path.display());
    println!("============================================================");

    Ok(())
}
