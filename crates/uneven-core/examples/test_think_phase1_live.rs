//! Standalone test binary to execute a single live Gemini LLM call
//! testing Phase 1 (Drafting) of uneven-core's `think` reasoning engine.
//!
//! Uses ONLY `GOOGLE_API_KEY` (or `GOOGLE_API_KEY_FREE` / `GEMINI_API_KEY`) from `.env`.
//! No parsing or local modification is performed — purely inspecting the raw AI thought output.
//!
//! Run with:
//!   cargo run -p uneven-core --example test_think_phase1_live
//! Or with a custom task:
//!   cargo run -p uneven-core --example test_think_phase1_live -- "sua tarefa aqui"

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
    println!("  Uneven Core — Live Think Phase 1 (Drafting) Benchmark");
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

    // 2. Load Phase 1 System Rules (Snatchy - Drafting / Actor)
    let draft_base = laws::get_context("think_draft");
    let draft_system = format!(
        "{}\n\n[PHASE: DRAFTING - Produce an initial concrete solution, architecture plan, and preliminary diffs.]",
        draft_base
    );
    println!("📜 System Rules loaded from `laws::get_context(\"think_draft\")`");

    // 3. Realistic Codebase Context Simulation
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
    // Current unbounded default connection pool without backoff or connection limits
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
        // Direct query executed per incoming request
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

    // 4. Task Input (from previous intent classification test args)
    let default_task = "Analyze the authentication gateway's database connection pooling in `src/infrastructure/db.rs` and `src/services/auth.rs` for leaks or undefined limits. Propose a refactoring to implement secure asynchronous pooling with exponential backoff and monitoring metrics to address PostgreSQL connection exhaustion under high concurrent load.";

    let task = std::env::args().nth(1).unwrap_or_else(|| default_task.to_string());

    println!("\n📝 Tarefa Submetida (Args da Fase 1):\n   \"{}\"", task);

    let draft_prompt = format!(
        "{}\nUser Request: {}\n\nFormulate a preliminary solution plan and the necessary code changes.",
        context_block, task
    );

    // 5. Configure LLM Client (Gemini 2.5 Flash, Temperature = 0.2, Max Tokens = 2500)
    let model_name = "gemini-2.5-flash";
    let client = LlmClient::builder(Provider::Gemini)
        .model(model_name)
        .api_key(&api_key)
        .build()?;

    let max_tokens = 2500;
    let params = CompletionParams::new(&draft_prompt)
        .with_system(&draft_system)
        .with_temperature(0.2)
        .with_max_tokens(max_tokens);

    println!("\n⚙️  Configuração da Chamada:");
    println!("   - Modelo:       {}", model_name);
    println!("   - Temperatura:  0.2 (temperatura oficial da Fase 1 - Drafting)");
    println!("   - Max Tokens:   {} (ampliado para permitir plano detalhado e diffs)", max_tokens);

    println!("\n🚀 Enviando 1 chamada controlada para o Google Gemini...");
    let start = Instant::now();
    let response = client.complete_with_params(params).await?;
    let duration = start.elapsed();

    println!("✅ Resposta recebida em {:.2?}!", duration);

    // 6. Telemetria
    if let Some(usage) = &response.usage {
        println!("\n📊 Telemetria de Tokens:");
        println!("   - Tokens de Entrada:  {}", usage.prompt_tokens);
        println!("   - Tokens de Saída:    {}", usage.completion_tokens);
        println!("   - Total de Tokens:    {}", usage.total_tokens);
    }
    if let Some(cost) = response.cost_usd {
        println!("   - Custo Estimado:     ${:.6} USD", cost);
    }

    // 7. Exibir Resposta Bruta Exatamente Como Recebida (Sem Parser)
    println!("\n------------------------------------------------------------");
    println!("💬 RESPOSTA BRUTA DO THINK (FASE 1 - DRAFTING):\n");
    println!("{}", response.content.trim());
    println!("------------------------------------------------------------");

    // 8. Salvar Dump da Resposta Bruta em Disco
    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let raw_path = output_dir.join("test_think_draft_raw_response.md");

    let dump_content = format!(
        "# Gemini Raw Think Phase 1 (Drafting) Response Dump\n\n\
        - **Timestamp:** {}\n\
        - **Model:** {}\n\
        - **Temperature:** 0.2\n\
        - **Max Tokens:** {}\n\
        - **Latency:** {:.2?}\n\
        - **Prompt Tokens:** {}\n\
        - **Completion Tokens:** {}\n\
        - **Total Tokens:** {}\n\
        - **Cost USD:** ${:.6}\n\n\
        ## Task Input\n\n\
        ```\n{}\n```\n\n\
        ---\n\n\
        ## Raw LLM Output Content\n\n\
        {}\n",
        chrono::Local::now().to_rfc3339(),
        model_name,
        max_tokens,
        duration,
        response.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
        response.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
        response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
        response.cost_usd.unwrap_or(0.0),
        task,
        response.content.trim(),
    );

    std::fs::write(&raw_path, &dump_content)?;
    println!("💾 Resposta bruta salva em: {}", raw_path.display());
    println!("============================================================");

    Ok(())
}
