//! Standalone test binary to execute a single live Gemini LLM call
//! testing Turn 2: Internal Critic & Contextual Pre-Flight Audit (`think_audit`)
//! of uneven-core's `think` reasoning engine.
//!
//! Feeds the exact Phase 1 (Drafting) raw output into the Critic,
//! with increased max_tokens (4000) and temperature = 0.2.
//!
//! Uses ONLY `GOOGLE_API_KEY` (or `GOOGLE_API_KEY_FREE` / `GEMINI_API_KEY`) from `.env`.
//! No parsing is performed — purely inspecting the raw AI critic audit output.
//!
//! Run with:
//!   cargo run -p uneven-core --example test_think_phase2_critic_live

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
    println!("  Uneven Core — Live Think Turn 2 (Critic Audit) Benchmark");
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

    // 2. Load Turn 2 System Rules (The Senior Auditor & Pre-Flight Critic)
    let critic_system = laws::get_context("think_audit");
    println!("📜 System Rules loaded from `laws::get_context(\"think_audit\")`");

    // 3. Load Phase 1 Draft Response from disk
    let draft_dump_path = PathBuf::from("target/reports/test_think_draft_raw_response.md");
    let draft_content = if draft_dump_path.exists() {
        let raw = std::fs::read_to_string(&draft_dump_path)?;
        if let Some((_, content)) = raw.split_once("## Raw LLM Output Content\n\n") {
            content.trim().to_string()
        } else {
            raw
        }
    } else {
        eprintln!("⚠️ Warning: Phase 1 draft file not found, using embedded reference.");
        "Preliminary draft proposing DbConfig and PgPoolOptions with backoff.".to_string()
    };
    println!("📥 Phase 1 Draft Proposal loaded ({} bytes)", draft_content.len());

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

    let critic_prompt = format!(
        "### User Original Request:\n{}\n\n### Codebase Context:\n{}\n\n### Initial Solution Draft:\n{}\n\nPerform the contextual self-critique and audit, enumerating any required corrections.",
        task, context_block, draft_content
    );

    // 5. Configure LLM Client with Significantly Higher Max Tokens (4000)
    let model_name = "gemini-2.5-flash";
    let client = LlmClient::builder(Provider::Gemini)
        .model(model_name)
        .api_key(&api_key)
        .build()?;

    let max_tokens = 4000;
    let params = CompletionParams::new(&critic_prompt)
        .with_system(&critic_system)
        .with_temperature(0.2)
        .with_max_tokens(max_tokens);

    println!("\n⚙️  Configuração da Chamada:");
    println!("   - Modelo:       {}", model_name);
    println!("   - Temperatura:  0.2 (temperatura de auditoria)");
    println!("   - Max Tokens:   {} (ampliado significativamente para auditoria sem cortes)", max_tokens);

    println!("\n🚀 Enviando 1 chamada controlada para o Google Gemini...");
    let start = Instant::now();
    let response = client.complete_with_params(params).await?;
    let duration = start.elapsed();

    println!("✅ Resposta do Auditor recebida em {:.2?}!", duration);

    // 6. Telemetria
    if let Some(usage) = &response.usage {
        println!("\n📊 Telemetria de Tokens:");
        println!("   - Tokens de Entrada (Prompt + Draft): {}", usage.prompt_tokens);
        println!("   - Tokens de Saída (Auditoria / Crítica): {}", usage.completion_tokens);
        println!("   - Total de Tokens:                    {}", usage.total_tokens);
    }
    if let Some(cost) = response.cost_usd {
        println!("   - Custo Estimado:                     ${:.6} USD", cost);
    }

    // 7. Exibir Resposta Bruta do Auditor (Sem Parser)
    println!("\n------------------------------------------------------------");
    println!("💬 RESPOSTA BRUTA DO AUDITOR CRÍTICO (TURN 2 - THINK AUDIT):\n");
    println!("{}", response.content.trim());
    println!("------------------------------------------------------------");

    // 8. Salvar Dump da Auditoria em Disco
    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let raw_path = output_dir.join("test_think_critic_raw_response.md");

    let dump_content = format!(
        "# Gemini Raw Think Turn 2 (Critic Audit) Response Dump\n\n\
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
        ## Raw Critic Audit Output Content\n\n\
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
    println!("💾 Resposta bruta da auditoria salva em: {}", raw_path.display());
    println!("============================================================");

    Ok(())
}
