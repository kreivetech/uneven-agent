//! Standalone test binary to execute a single live Gemini LLM call
//! testing uneven-core's database analysis prompt, schema masking,
//! ResponseParser, and HTML dashboard compilation.
//!
//! Uses ONLY `GOOGLE_API_KEY` (or `GOOGLE_API_KEY_FREE` / `GEMINI_API_KEY`) from `.env`.
//! No persistent database mutation is performed.
//!
//! Run with:
//!   cargo run -p uneven-core --example test_analyze_live

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use llmcraft::{CompletionParams, LlmClient, Provider};
use serde_json::json;
use uneven_core::domain::{ColumnInfo, ForeignKey, TableSchema};
use uneven_core::infrastructure::knowledge::prompts::build_sql_gen_prompt;
use uneven_core::infrastructure::knowledge::{
    build_prompt_rules, format_schema_for_ai, ResponseParser,
};
use uneven_core::infrastructure::reporting::generate_dashboard;
use uneven_core::infrastructure::security::SecurityContext;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load environment variables (.env from current or parent workspace directory)
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_path(PathBuf::from("../../.env"));
    }

    println!("============================================================");
    println!("  Uneven Core — Live Gemini Analysis & Dashboard Benchmark");
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

    // 2. Prepare Fictional Database Schema
    println!("\n📦 Preparing Fictional Operational Schema...");
    let customers_table = TableSchema {
        name: "customers".to_string(),
        columns: vec![
            ColumnInfo { name: "customer_id".to_string(), r#type: "INTEGER".to_string(), nullable: false },
            ColumnInfo { name: "company_name".to_string(), r#type: "TEXT".to_string(), nullable: false },
            ColumnInfo { name: "segment".to_string(), r#type: "TEXT".to_string(), nullable: true },
            ColumnInfo { name: "country".to_string(), r#type: "TEXT".to_string(), nullable: false },
        ],
        row_count: Some(150),
        samples: Some(vec![
            json!({"customer_id": 1, "company_name": "Acme Corp", "segment": "Enterprise", "country": "BR"}),
            json!({"customer_id": 2, "company_name": "Globex Inc", "segment": "SMB", "country": "US"}),
        ]),
        foreign_keys: None,
    };

    let orders_table = TableSchema {
        name: "orders".to_string(),
        columns: vec![
            ColumnInfo { name: "order_id".to_string(), r#type: "INTEGER".to_string(), nullable: false },
            ColumnInfo { name: "customer_id".to_string(), r#type: "INTEGER".to_string(), nullable: false },
            ColumnInfo { name: "region".to_string(), r#type: "TEXT".to_string(), nullable: false },
            ColumnInfo { name: "total_amount".to_string(), r#type: "REAL".to_string(), nullable: false },
            ColumnInfo { name: "status".to_string(), r#type: "TEXT".to_string(), nullable: false },
            ColumnInfo { name: "created_at".to_string(), r#type: "TEXT".to_string(), nullable: false },
            // Blocked column to verify Blind Shield filtering:
            ColumnInfo { name: "credit_card_token".to_string(), r#type: "TEXT".to_string(), nullable: true },
        ],
        row_count: Some(1250),
        samples: Some(vec![
            json!({"order_id": 101, "customer_id": 1, "region": "Sudeste", "total_amount": 15400.0, "status": "completed", "created_at": "2026-03-10", "credit_card_token": "tok_sec_9948"}),
            json!({"order_id": 102, "customer_id": 2, "region": "Sul", "total_amount": 8900.5, "status": "completed", "created_at": "2026-03-11", "credit_card_token": "tok_sec_1120"}),
        ]),
        foreign_keys: Some(vec![
            ForeignKey {
                column: "customer_id".to_string(),
                referenced_table: "customers".to_string(),
                referenced_column: "customer_id".to_string(),
            }
        ]),
    };

    let schemas = vec![customers_table, orders_table];

    // 3. Security Context & Schema Formatting
    let security = Arc::new(SecurityContext::new(
        vec!["credit_card_token".to_string(), "password".to_string()],
        vec!["secret_audit_log".to_string()],
        vec!["email".to_string()],
    ));

    let schema_text = format_schema_for_ai(&schemas, &security);
    let prompt_rules = build_prompt_rules(&security, "");
    let user_inquiry = "Quais são as principais regiões comerciais por faturamento total e número de pedidos concluídos?";

    println!("🔒 Schema masked under Blind Shield policies.");
    println!("📝 User Inquiry: \"{}\"", user_inquiry);

    let full_prompt = build_sql_gen_prompt(
        Some("SQLite"),
        &schema_text,
        user_inquiry,
        &prompt_rules,
    );

    // 4. Single Live LLM Call via llmcraft
    let model_name = "gemini-2.5-flash";
    let client = LlmClient::builder(Provider::Gemini)
        .model(model_name)
        .api_key(&api_key)
        .build()?;

    let mut params = CompletionParams::new(&full_prompt);
    params.max_tokens = Some(700);

    println!("\n🚀 Sending 1 controlled live request to Google Gemini ({model_name})...");
    let start = Instant::now();
    let response = client.complete_with_params(params).await?;
    let duration = start.elapsed();

    println!("✅ Response received in {:.2?}!", duration);

    // Save Raw Response to disk for manual inspection
    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let raw_response_path = output_dir.join("test_analyze_raw_response.md");

    let mut raw_dump = format!(
        "# Gemini Raw AI Response Dump\n\n\
        - **Timestamp:** {}\n\
        - **Model:** {}\n\
        - **Latency:** {:.2?}\n",
        chrono::Local::now().to_rfc3339(),
        model_name,
        duration,
    );

    // Telemetry
    if let Some(usage) = &response.usage {
        println!("📊 Token Telemetry:");
        println!("   - Input Tokens:  {}", usage.prompt_tokens);
        println!("   - Output Tokens: {}", usage.completion_tokens);
        println!("   - Total Tokens:  {}", usage.total_tokens);

        raw_dump.push_str(&format!(
            "- **Prompt Tokens:** {}\n\
            - **Completion Tokens:** {}\n\
            - **Total Tokens:** {}\n",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
        ));
    }
    if let Some(cost) = response.cost_usd {
        println!("   - Cost:          ${:.6} USD", cost);
        raw_dump.push_str(&format!("- **Cost USD:** ${:.6}\n", cost));
    }

    raw_dump.push_str("\n---\n\n## Raw LLM Output Content\n\n");
    raw_dump.push_str(&response.content);

    std::fs::write(&raw_response_path, &raw_dump)?;
    println!("💾 Raw response saved to: {}", raw_response_path.display());

    // 5. ResponseParser Validation
    println!("\n------------------------------------------------------------");
    println!("🔍 Parsing Response with uneven-core ResponseParser...");
    let parsed = ResponseParser::parse(&response.content);

    println!("   - Cannot Generate: {}", parsed.cannot_generate);
    println!("   - SQL Query:\n{}", if parsed.sql.is_empty() { "   [NONE]" } else { &parsed.sql });
    println!("   - Explanation: {}", parsed.explanation);
    if !parsed.executive_report.is_empty() {
        println!("   - Executive Report: {}", parsed.executive_report);
    }

    if parsed.cannot_generate || parsed.sql.is_empty() {
        eprintln!("❌ Failed: AI could not generate a valid SQL query.");
        std::process::exit(1);
    }

    // Audit SQL safety
    let is_safe = shieldcraft::is_safe_query(&parsed.sql) && security.is_safe_query(&parsed.sql);
    println!("   - SQL Safe (read-only): {}", if is_safe { "YES ✅" } else { "NO ❌" });
    if !is_safe {
        eprintln!("❌ Security failure: Query contains destructive statements.");
        std::process::exit(1);
    }

    // 6. Dashboard Compilation Verification
    println!("\n📊 Simulating local execution & compiling interactive HTML Dashboard...");
    let fictional_results = vec![
        json!({"region": "Sudeste", "total_amount": 145000.0, "pedidos_count": 820}),
        json!({"region": "Sul", "total_amount": 92000.0, "pedidos_count": 510}),
        json!({"region": "Nordeste", "total_amount": 68000.0, "pedidos_count": 390}),
        json!({"region": "Centro-Oeste", "total_amount": 45000.0, "pedidos_count": 270}),
        json!({"region": "Norte", "total_amount": 29000.0, "pedidos_count": 180}),
    ];

    let dashboard_html = generate_dashboard(
        &fictional_results,
        if !parsed.explanation.is_empty() { &parsed.explanation } else { "Regional Performance Audit" },
        "dark",
        parsed.visual_suggestions.as_ref(),
    );

    // Save dashboard to disk
    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("test_analyze_dashboard.html");
    std::fs::write(&output_path, &dashboard_html)?;

    let metadata = std::fs::metadata(&output_path)?;
    let file_size_kb = metadata.len() as f64 / 1024.0;

    println!("✅ Dashboard HTML generated successfully!");
    println!("   - Path: {}", output_path.display());
    println!("   - Size: {:.2} KB", file_size_kb);

    assert!(metadata.len() > 500, "Dashboard HTML file size is suspiciously small");
    assert!(dashboard_html.contains("<html") || dashboard_html.contains("<!DOCTYPE html>"), "Dashboard does not contain valid HTML");

    println!("\n🎉 Benchmark & Verification Completed Successfully!");
    println!("============================================================");

    Ok(())
}
