//! Standalone test binary to execute a single live Gemini LLM call
//! testing the interactive shell intent classifier under real shell laws,
//! temperature = 0.1, and displaying the pure raw AI decision.
//!
//! Uses ONLY `GOOGLE_API_KEY` (or `GOOGLE_API_KEY_FREE` / `GEMINI_API_KEY`) from `.env`.
//! No parsing or local modification is performed.
//!
//! Run with:
//!   cargo run -p uneven-core --example test_classify_live
//! Or with a custom prompt:
//!   cargo run -p uneven-core --example test_classify_live -- "seu prompt aqui"

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
    println!("  Uneven Core — Live Gemini Shell Intent Benchmark");
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

    // 2. Load Real Interactive Shell Rules and System Context
    let system_instructions = laws::get_context("shell");
    println!("📜 System Rules loaded from `laws::get_context(\"shell\")`");

    // 3. User Prompt (complex scenario or custom argument)
    let default_complex_prompt = "Estamos tendo um problema crítico de performance no gateway de autenticação: sob alta carga de requisições concorrentes, as conexões com o PostgreSQL se esgotam e causam timeout nas requisições. Analise a arquitetura atual em `src/infrastructure/db.rs` e `src/services/auth.rs`, verifique se o pool de conexões está com leak ou sem limite definido, e proponha a refatoração necessária para implementar pooling assíncrono seguro com backoff exponencial e métricas de monitoramento.";

    let user_prompt = std::env::args().nth(1).unwrap_or_else(|| default_complex_prompt.to_string());

    println!("\n📝 Prompt Submetido:\n   \"{}\"", user_prompt);

    // Mock project files map as provided in the interactive shell
    let files_map = "src/main.rs\nsrc/infrastructure/db.rs\nsrc/services/auth.rs\nCargo.toml\n";
    let final_prompt = format!("PROJECT FILES MAP (Relative paths):\n{}\n{}", files_map, user_prompt);

    // 4. Configure LLM Client (Gemini 2.5 Flash, Temperature = 0.1)
    let model_name = "gemini-2.5-flash";
    let client = LlmClient::builder(Provider::Gemini)
        .model(model_name)
        .api_key(&api_key)
        .build()?;

    let params = CompletionParams::new(&final_prompt)
        .with_system(&system_instructions)
        .with_temperature(0.1)
        .with_max_tokens(600);

    println!("\n⚙️  Configuração da Chamada:");
    println!("   - Modelo:       {}", model_name);
    println!("   - Temperatura:  0.1 (idêntica ao classificador interativo)");
    println!("   - Max Tokens:   600");

    println!("\n🚀 Enviando 1 chamada controlada para o Google Gemini...");
    let start = Instant::now();
    let response = client.complete_with_params(params).await?;
    let duration = start.elapsed();

    println!("✅ Resposta recebida em {:.2?}!", duration);

    // 5. Telemetria
    if let Some(usage) = &response.usage {
        println!("\n📊 Telemetria de Tokens:");
        println!("   - Tokens de Entrada:  {}", usage.prompt_tokens);
        println!("   - Tokens de Saída:    {}", usage.completion_tokens);
        println!("   - Total de Tokens:    {}", usage.total_tokens);
    }
    if let Some(cost) = response.cost_usd {
        println!("   - Custo Estimado:     ${:.6} USD", cost);
    }

    // 6. Exibir Resposta Bruta Exatamente Como Recebida
    println!("\n------------------------------------------------------------");
    println!("💬 RESPOSTA BRUTA DA IA (SEM PARSER):\n");
    println!("{}", response.content.trim());
    println!("------------------------------------------------------------");

    // 7. Salvar Dump da Resposta Bruta em Disco
    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let raw_path = output_dir.join("test_classify_raw_response.md");

    let dump_content = format!(
        "# Gemini Raw Shell Intent Response Dump\n\n\
        - **Timestamp:** {}\n\
        - **Model:** {}\n\
        - **Temperature:** 0.1\n\
        - **Latency:** {:.2?}\n\
        - **Prompt Tokens:** {}\n\
        - **Completion Tokens:** {}\n\
        - **Total Tokens:** {}\n\
        - **Cost USD:** ${:.6}\n\n\
        ## Prompt Submetido\n\n\
        ```\n{}\n```\n\n\
        ---\n\n\
        ## Raw LLM Output Content\n\n\
        {}\n",
        chrono::Local::now().to_rfc3339(),
        model_name,
        duration,
        response.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
        response.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
        response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
        response.cost_usd.unwrap_or(0.0),
        user_prompt,
        response.content.trim(),
    );

    std::fs::write(&raw_path, &dump_content)?;
    println!("💾 Resposta bruta salva em: {}", raw_path.display());
    println!("============================================================");

    Ok(())
}
