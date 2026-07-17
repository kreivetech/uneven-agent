use anyhow::Result;
use std::io::Write;
use std::path::Path;
use serde_json::Value;

use uneven_core::domain::TableSchema;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::security::{laws, SecurityContext};
use uneven_core::infrastructure::knowledge::DatabaseAnalyst;
use uneven_core::infrastructure::knowledge::parser::ResponseParser;
use uneven_core::infrastructure::knowledge::prompts::{
    build_analysis_prompt, build_sql_gen_prompt, build_style_extraction_prompt,
};
use uneven_core::infrastructure::llm::{InferParams, init as init_llm};
use uneven_core::infrastructure::reporting::{ExcelReport, generate_dashboard, package_to_exe};

#[derive(serde::Serialize, serde::Deserialize)]
struct AnalystTask {
    id: String,
    #[serde(rename = "tableName")]
    table_name: String,
    status: String, // "pending", "completed", "failed"
    attempts: u32,
    #[serde(rename = "resultPath")]
    result_path: Option<String>,
    error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AnalystJob {
    id: String,
    #[serde(rename = "startTime")]
    start_time: i64,
    goal: String,
    #[serde(rename = "dbUrl")]
    db_url: String,
    tasks: Vec<AnalystTask>,
}

#[allow(clippy::too_many_arguments)]
#[allow(deprecated)]
pub async fn run(
    db: Option<String>,
    prompt: Option<String>,
    report: String,
    theme: String,
    package_exe: bool,
    clear_cache: bool,
    provider: Option<String>,
    model: Option<String>,
) -> Result<()> {
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("analyze");

    if let Some(ref p) = provider {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model {
        brain.model = m.clone();
    }

    if clear_cache {
        let cache_dir = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("analyst");
        let mut count = 0;
        if cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_file()).unwrap_or(false)
                        && std::fs::remove_file(entry.path()).is_ok()
                    {
                        count += 1;
                    }
                }
            }
        }
        println!(
            "{}",
            crate::ui::truecolor(&format!("\n  ✓ Analyst cache cleared ({} files removed)", count), 0, 200, 100)
        );
        return Ok(());
    }

    // License & Limit validation for Free tier
    if !super::license::check_pro() {
        if theme != "dark" {
            anyhow::bail!("\n  [UE-01000005] Theme customization requires a Pro license. Only the default 'dark' theme is available on Free tier.\n");
        }
        let usage = super::license::read_usage().unwrap_or_else(|| {
            super::license::UsageData::new_empty(chrono::Local::now().format("%Y-%m-%d").to_string())
        });
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if usage.date == today && usage.analyze_count >= 3 {
            anyhow::bail!("\n  [UE-02000003] Daily limit of 3 queries for 'analyze' reached on Free license.\n\n  Upgrade to Pro for unlimited use: https://uneven.dev/pricing\n");
        }
    }

    let db_url = db.clone()
        .or_else(|| config.knowledge.as_ref().and_then(|k| k.db.as_ref().and_then(|d| d.url.clone())))
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .or_else(|| std::env::var("UNEVEN_DB_URL").ok());

    let db_url = match db_url {
        Some(url) => url,
        None => {
            anyhow::bail!("\n  ✗ No database URL provided.\n  Use: uneven analyze --db <url>\n  Or:  export DATABASE_URL=<url>");
        }
    };

    let session_id = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let report_dir = std::env::current_dir()?.join(".uneven").join("reports").join("analysis").join(&session_id);
    std::fs::create_dir_all(&report_dir)?;

    let allowed_tables = config.knowledge.as_ref()
        .and_then(|k| k.db.as_ref())
        .and_then(|db| {
            db.allowed_tables.clone()
                .or_else(|| db.tables.clone())
        })
        .unwrap_or_default();

    if !super::license::check_pro() && allowed_tables.len() > 5 {
        anyhow::bail!("\n  [UE-01000005] Table analysis limit exceeded. Free license allows analyzing up to 5 tables. Please upgrade to Pro for unlimited use.\n");
    }

    if allowed_tables.is_empty() {
        println!("{}", crate::ui::rust("\n  [UE-03000012] ✗ Database access restricted."));
        println!("{}", crate::ui::dim("  To analyze your database, you must explicitly list allowed tables. Please use '/' in the interactive shell."));
        anyhow::bail!("[UE-03000012] Database access blocked: No whitelisted tables found. Please configure them using '/' in the interactive shell.");
    }

    let blocked_columns = config.security.blocked_columns.clone();
    let blocked_tables = config.security.blocked_tables.clone();
    let masked_columns = config.security.masked_columns.clone();
    let security = std::sync::Arc::new(uneven_core::infrastructure::security::SecurityContext::new(
        blocked_columns,
        blocked_tables,
        masked_columns,
    ));


    let mut analyst = DatabaseAnalyst::new(security.clone(), allowed_tables);

    let system_instructions = laws::get_context("analyze");
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    
    // Initialize LLM
    init_llm(&brain.provider, &brain.model, threads).await?;

    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 2048,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_instructions.clone()),
        history: None,
    };

    let db_type = if db_url.starts_with("sqlite") {
        Some("sqlite")
    } else if db_url.starts_with("postgres") || db_url.starts_with("pg") {
        Some("postgres")
    } else if db_url.starts_with("mysql") {
        Some("mysql")
    } else if db_url.starts_with("mongodb") {
        Some("mongodb")
    } else if db_url.starts_with("redis") {
        Some("redis")
    } else {
        None
    };

    // Automated flow if prompt is provided
    if let Some(ref prompt_str) = prompt {
        if !super::license::check_pro() {
            let c = super::license::check_and_increment_usage("analyze")?;
            println!(
                "\n{}",
                crate::ui::truecolor(&format!("  ℹ Free tier: {} of 3 daily queries used.", c), 168, 200, 255)
            );
        }

        println!("{}", crate::ui::truecolor(&format!("\n  🚀 Automated Analysis: \"{}\"", prompt_str), 168, 200, 255));
        analyst.connect(&db_url).await?;
        println!("  Connected to {}", db_type.unwrap_or("SQL"));

        let schema = analyst.introspect(None).await?;
        if !super::license::check_pro() && schema.len() > 5 {
            anyhow::bail!("\n  [UE-01000005] Table analysis limit exceeded. Free license allows analyzing up to 5 tables. Please upgrade to Pro for unlimited use.\n");
        }
        let schema_text = format_schema_for_ai(&schema);
        let prompt_rules = build_prompt_rules(&security, &system_instructions);

        let sql_prompt = build_sql_gen_prompt(
            db_type,
            &schema_text,
            prompt_str,
            &prompt_rules,
        );

        let gen_response = uneven_core::infrastructure::llm::infer_with_params(&sql_prompt, &params).await?;
        let generated = ResponseParser::parse(&gen_response.content);

        if generated.cannot_generate {
            println!("\n  ✗ Could not generate query: {}", generated.explanation);
            return Ok(());
        }

        println!("  Executing generated query…");
        let rows = analyst.execute_query(&generated.sql).await?;

        let columns: Vec<String> = if !rows.is_empty() {
            if let Value::Object(map) = &rows[0] {
                map.keys().cloned().collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let label = prompt_str.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
        let label_truncated = if label.len() > 40 { &label[..40] } else { &label };
        let report_type = report.to_lowercase();

        if report_type == "html" || report_type == "both" {
            let visual_suggestions = if !super::license::check_pro() {
                None
            } else {
                generated.visual_suggestions.as_ref()
            };
            let html = generate_dashboard(
                &rows,
                prompt_str,
                &theme,
                visual_suggestions,
            );
            let html_filename = format!("auto-report-{}.html", label_truncated);
            let html_path = report_dir.join(&html_filename);
            std::fs::write(&html_path, &html)?;
            println!(
                "{}",
                crate::ui::truecolor(&format!("\n  ✓ Dashboard generated: {}", html_filename), 0, 200, 100)
            );

            if package_exe {
                println!("  Building executable...");
                let exe_filename = format!("auto-report-{}.exe", label_truncated);
                let exe_path = report_dir.join(&exe_filename);
                if let Err(e) = package_to_exe(&html, prompt_str, &exe_path) {
                    println!("  ✗ Exe build failed: {}", e);
                } else {
                    println!(
                        "{}",
                        crate::ui::truecolor(&format!("  ✓ Executable created: {}", exe_filename), 0, 200, 100)
                    );
                }
            }
        }

        if report_type == "excel" || report_type == "both" {
            let excel_filename = format!("auto-report-{}.xlsx", label_truncated);
            let excel_path = report_dir.join(&excel_filename);
            let mut workbook = ExcelReport::new();
            workbook.add_sheet("Analysis", &columns, &rows)?;
            workbook.save(&excel_path)?;
            println!(
                "{}",
                crate::ui::truecolor(&format!("  ✓ Excel report saved: {}", excel_filename), 0, 200, 100)
            );
        }

        return Ok(());
    }

    println!("\n  ⚠  SECURE CONNECTION PROTOCOL");
    print!("{}", crate::ui::truecolor("  Confirm network stability? [Y/n] > ", 0, 220, 220));
    std::io::stdout().flush()?;
    let mut net_confirm = String::new();
    std::io::stdin().read_line(&mut net_confirm)?;
    if net_confirm.trim().to_lowercase() == "n" {
        return Ok(());
    }

    print_header();
    print!("  Establishing initial connection…");
    std::io::stdout().flush()?;
    analyst.connect(&db_url).await?;
    println!("{}", crate::ui::truecolor(&format!(" connected to {}\n", db_type.unwrap_or("SQL")), 0, 200, 100));

    let all_table_names = analyst.list_tables().await?;
    println!("  {} tables found\n", all_table_names.len());

    let mut selected_tables = None;
    if !all_table_names.is_empty() {
        selected_tables = prompt_table_selection(&all_table_names)?;
    }

    if !super::license::check_pro() {
        let selected_count = selected_tables.as_ref().map(|s| s.len()).unwrap_or(all_table_names.len());
        if selected_count > 5 {
            anyhow::bail!("\n  [UE-01000005] Table analysis limit exceeded. Free license allows analyzing up to 5 tables. Please upgrade to Pro for unlimited use.\n");
        }
    }

    if let Some(ref selected) = selected_tables {
        if !selected.is_empty() {
            print!("\n  Generate Auto-Dashboard for selected tables? [y/N] > ");
            std::io::stdout().flush()?;
            let mut run_auto = String::new();
            std::io::stdin().read_line(&mut run_auto)?;
            if run_auto.trim().to_lowercase() == "y" {
                if !super::license::check_pro() {
                    anyhow::bail!("\n  [UE-01000005] Batch dashboard generation requires a Pro license. Please upgrade to Pro.\n");
                }
                run_batch_dashboard(&mut analyst, &security, selected, &report_dir, &db_url, package_exe).await?;
                return Ok(());
            }
        }
    }

    let schema = analyst.introspect(selected_tables.clone()).await?;
    if !super::license::check_pro() && schema.len() > 5 {
        anyhow::bail!("\n  [UE-01000005] Table analysis limit exceeded. Free license allows analyzing up to 5 tables. Please upgrade to Pro for unlimited use.\n");
    }
    println!("  {} tables loaded\n", schema.len());

    print_schema(&schema);

    let suggestions = suggest_analyses(&schema, &config, &system_instructions, threads).await?;
    if !suggestions.is_empty() {
        println!("{}", crate::ui::truecolor("  Suggested analyses:", 168, 200, 255));
        for (i, s) in suggestions.iter().enumerate() {
            println!(
                "    {} {}",
                crate::ui::truecolor(&format!("{}.", i + 1), 128, 128, 128),
                crate::ui::truecolor(s, 232, 237, 245)
            );
        }
        println!();
    }

    let mut query_index = 0;
    let mut session_results: Vec<(Vec<Value>, Vec<String>, String)> = vec![];
    let mut history: Vec<(String, String)> = Vec::new();

    loop {
        print!("{}", crate::ui::truecolor("  > ", 0, 220, 220));
        std::io::stdout().flush()?;
        let mut user_request = String::new();
        std::io::stdin().read_line(&mut user_request)?;
        let user_request = user_request.trim().to_string();
        if user_request.is_empty() || user_request.to_lowercase() == "quit" || user_request.to_lowercase() == "q" {
            break;
        }
        if !super::license::check_pro() {
            let c = super::license::check_and_increment_usage("analyze")?;
            println!(
                "{}",
                crate::ui::truecolor(&format!("  ℹ Free tier: {} of 3 daily queries used.", c), 168, 200, 255)
            );
        }
        if user_request.to_lowercase() == "schema" {
            print_schema(&schema);
            continue;
        }

        let mut effective_request = user_request.clone();
        if let Ok(num) = user_request.parse::<usize>() {
            if num >= 1 && num <= suggestions.len() {
                effective_request = suggestions[num - 1].clone();
            }
        }

        query_index += 1;
        println!("  Generating SQL...");

        let schema_text = format_schema_for_ai(&schema);
        let prompt_rules = build_prompt_rules(&security, &system_instructions);

        let mut loop_params = InferParams {
            model: brain.model.clone(),
            threads,
            max_tokens: brain.max_tokens,
            temperature: brain.temperature as f64,
            repeat_penalty: 1.1,
            max_context: 2048,
            grammar: None,
            provider: Some(brain.provider.clone()),
            system: Some(system_instructions.clone()),
            history: None,
        };

        let sql_prompt = if brain.provider != "local" {
            let base_prompt = build_sql_gen_prompt(
                db_type,
                "",
                &effective_request,
                "",
            );
            let optimized_prompt = uneven_core::infrastructure::llm::history_manager::optimize_context(
                &mut history,
                &base_prompt,
                &prompt_rules,
                Some(&schema_text),
            );
            loop_params.system = None;
            loop_params.history = Some(history.clone());
            optimized_prompt
        } else {
            build_sql_gen_prompt(
                db_type,
                &schema_text,
                &effective_request,
                &prompt_rules,
            )
        };

        let gen_response = uneven_core::infrastructure::llm::infer_with_params(&sql_prompt, &loop_params).await?;
        let generated = ResponseParser::parse(&gen_response.content);

        if generated.cannot_generate {
            println!("  ⚠ Could not generate query: {}\n", generated.explanation);
            continue;
        }

        if brain.provider != "local" {
            history.push(("user".to_string(), sql_prompt.clone()));
            history.push(("assistant".to_string(), gen_response.content.clone()));
            let history_limit = if brain.provider == "local" { 8 } else { 14 };
            if history.len() > history_limit {
                history.drain(0..history.len() - history_limit);
            }
        }

        println!("  Generated SQL:");
        print_sql_box(&generated.sql);

        let mut sql_to_run = generated.sql.clone();
        print!("{}", crate::ui::truecolor("  [A]pprove  [E]dit  [S]kip > ", 255, 255, 0));
        std::io::stdout().flush()?;
        let mut approval = String::new();
        std::io::stdin().read_line(&mut approval)?;
        let approval = approval.trim().to_uppercase();

        if approval == "S" || approval == "SKIP" {
            continue;
        }

        if approval == "E" || approval == "EDIT" {
            println!("  Paste your SQL (end with a blank line):");
            let mut lines = vec![];
            loop {
                let mut line = String::new();
                std::io::stdin().read_line(&mut line)?;
                if line.trim().is_empty() {
                    break;
                }
                lines.push(line);
            }
            sql_to_run = lines.join("\n").trim().to_string();
            if sql_to_run.is_empty() {
                continue;
            }
            print_sql_box(&sql_to_run);
            print!("{}", crate::ui::truecolor("  Confirm execution? [Y/n] > ", 255, 255, 0));
            std::io::stdout().flush()?;
            let mut confirm = String::new();
            std::io::stdin().read_line(&mut confirm)?;
            if confirm.trim().to_lowercase() == "n" {
                continue;
            }
        } else if approval != "A" && approval != "APPROVE" && !approval.is_empty() {
            continue;
        }

        if !is_safe_query(&sql_to_run) {
            println!("\n  🚫 BLOCKED: Only SELECT statements are permitted.\n");
            continue;
        }

        println!("  Executing...");
        let rows = match analyst.execute_query(&sql_to_run).await {
            Ok(r) => r,
            Err(e) => {
                println!("  ✗ Query failed: {}\n", e);
                continue;
            }
        };

        println!("  {} rows returned.\n", rows.len());
        if rows.is_empty() {
            continue;
        }

        let columns: Vec<String> = if let Value::Object(map) = &rows[0] {
            map.keys().cloned().collect()
        } else {
            vec![]
        };

        print_preview(&columns, &rows);

        let label = effective_request.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
        let label_truncated = if label.len() > 40 { &label[..40] } else { &label };
        session_results.push((rows.clone(), columns.clone(), format!("Q{}_{}", query_index, label_truncated)));

        print!("{}", crate::ui::truecolor("\n  [X]Excel  [D]Dashboard  [B]Both  [N]New query  [Q]uit > ", 168, 200, 255));
        std::io::stdout().flush()?;
        let mut choice = String::new();
        std::io::stdin().read_line(&mut choice)?;
        let choice = choice.trim().to_uppercase();

        if choice == "Q" {
            break;
        }

        if choice == "X" || choice == "B" {
            let excel_filename = format!("Q{}-{}.xlsx", query_index, label_truncated);
            let excel_path = report_dir.join(&excel_filename);
            let mut workbook = ExcelReport::new();
            workbook.add_sheet(&format!("Q{}", query_index), &columns, &rows)?;
            workbook.save(&excel_path)?;
            println!("  ✓ Excel report saved: {}", excel_path.display());
        }

        if choice == "D" || choice == "B" {
            let (final_theme, final_suggestions) = if !super::license::check_pro() {
                println!("{}", crate::ui::truecolor("  ℹ Dashboard theme customization requires a Pro license. Generating using default dark theme...", 168, 200, 255));
                ("dark".to_string(), None)
            } else {
                print!("{}", crate::ui::truecolor("\n  [Visual] Como deseja o visual do relatório? (Enter para usar o padrão) > ", 168, 200, 255));
                std::io::stdout().flush()?;
                let mut style_prompt = String::new();
                std::io::stdin().read_line(&mut style_prompt)?;
                let style_prompt = style_prompt.trim().to_string();

                let mut final_theme = theme.clone();
                let mut final_suggestions = generated.visual_suggestions.clone();

                if !style_prompt.is_empty() {
                    println!("  Extraindo customização visual...");
                    let custom_style_prompt = build_style_extraction_prompt(&style_prompt, &columns);
                    let style_params = InferParams {
                        model: brain.model.clone(),
                        threads,
                        max_tokens: brain.max_tokens,
                        temperature: brain.temperature as f64,
                        repeat_penalty: 1.1,
                        max_context: 2048,
                        grammar: None,
                        provider: Some(brain.provider.clone()),
                        system: Some(system_instructions.clone()),
                        history: None,
                    };
                    if let Ok(style_response) = uneven_core::infrastructure::llm::infer_with_params(&custom_style_prompt, &style_params).await {
                        if let Some(Value::Object(custom_map)) = ResponseParser::extract_theme(&style_response.content) {
                                let mut current_map = match final_suggestions {
                                    Some(Value::Object(m)) => m,
                                    _ => serde_json::Map::new(),
                                };
                                for (k, v) in custom_map {
                                    current_map.insert(k, v);
                                }
                                if let Some(theme_val) = current_map.get("themeMode").and_then(|t| t.as_str()) {
                                    final_theme = theme_val.to_string();
                                }
                                final_suggestions = Some(Value::Object(current_map));
                        }
                    }
                }
                (final_theme, final_suggestions)
            };

            let html = generate_dashboard(
                &rows,
                &effective_request,
                &final_theme,
                final_suggestions.as_ref(),
            );

            let html_filename = format!("Q{}-{}.html", query_index, label_truncated);
            let html_path = report_dir.join(&html_filename);
            std::fs::write(&html_path, &html)?;
            println!("  ✓ Dashboard generated: {}", html_path.display());

            if package_exe {
                println!("  Building executable...");
                let exe_filename = format!("Q{}-{}.exe", query_index, label_truncated);
                let exe_path = report_dir.join(&exe_filename);
                if let Err(e) = package_to_exe(&html, &effective_request, &exe_path) {
                    println!("  ✗ Exe build failed: {}", e);
                } else {
                    println!("  ✓ Executable created: {}", exe_path.display());
                }
            }
        }
    }

    if !session_results.is_empty() {
        print!("{}", crate::ui::truecolor("\n  Export all into one Excel workbook? [Y/n] > ", 0, 220, 220));
        std::io::stdout().flush()?;
        let mut ans = String::new();
        std::io::stdin().read_line(&mut ans)?;
        if ans.trim().to_lowercase() != "n" {
            let excel_filename = format!("analysis-session-{}.xlsx", session_id);
            let excel_path = report_dir.join(&excel_filename);
            let mut workbook = ExcelReport::new();
            for (rows, columns, sheet_name) in session_results {
                let safe_sheet_name = if sheet_name.len() > 30 { &sheet_name[..30] } else { &sheet_name };
                workbook.add_sheet(safe_sheet_name, &columns, &rows)?;
            }
            workbook.save(&excel_path)?;
            println!("  ✓ Combined workbook saved: {}", excel_path.display());
        }
    }

    println!("\n  Session ended.\n");
    Ok(())
}

fn format_schema_for_suggestion(schema: &[TableSchema]) -> String {
    let limit = schema.len().min(20);
    schema[..limit]
        .iter()
        .map(|t| {
            let cols = t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ");
            format!("  {}({})", t.name, cols)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_schema_for_ai(schema: &[TableSchema]) -> String {
    if schema.is_empty() {
        return "No tables available".to_string();
    }
    
    let limit = schema.len().min(20);
    let tables = &schema[..limit];
    let details: Vec<String> = tables.iter().map(|t| {
        let cols = t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(",");
        let fks_str = if let Some(ref fks) = t.foreign_keys {
            if !fks.is_empty() {
                let fks_mapped = fks.iter().map(|fk| {
                    format!("{}->{}.{}", fk.column, fk.referenced_table, fk.referenced_column)
                }).collect::<Vec<_>>().join(", ");
                format!(" | RELATIONSHIPS: {}", fks_mapped)
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };
        let samples_str = if let Some(ref samples) = t.samples {
            if !samples.is_empty() {
                format!(" | SAMPLE: {}", format_samples_for_ai(samples))
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };
        format!("{}{{{}}}{}{}", t.name, cols, fks_str, samples_str)
    }).collect();

    format!("CONTEXT (Tables & Relationships & Samples):\n{}", details.join("\n"))
}

fn format_samples_for_ai(samples: &[Value]) -> String {
    if samples.is_empty() {
        return "".to_string();
    }
    let first = &samples[0];
    if let Value::Object(map) = first {
        map.iter().map(|(k, v)| {
            let val_str = match v {
                Value::String(s) => {
                    if s.len() > 30 {
                        format!("{}...", &s[..30])
                    } else {
                        s.clone()
                    }
                }
                _ => v.to_string(),
            };
            format!("{}={}", k, val_str)
        }).collect::<Vec<_>>().join("|")
    } else {
        first.to_string()
    }
}

fn build_prompt_rules(
    security: &uneven_core::infrastructure::security::SecurityContext,
    blindados_laws: &str,
) -> String {
    let mut exact_cols = security.blocked_columns.clone();
    for col in uneven_core::infrastructure::security::masking::BLOCKED_COLUMNS {
        let col_str = col.to_string();
        if !exact_cols.contains(&col_str) {
            exact_cols.push(col_str);
        }
    }

    let mut lines = vec![
        "SECURITY RULES — you MUST follow these unconditionally:".to_string(),
        format!("- NEVER SELECT these BLOCKED columns (they are forbidden): {}", exact_cols.join(", ")),
        format!("- NEVER query these BLOCKED tables: {}", security.blocked_tables.join(", ")),
        "- If the user request requires any of the BLOCKED items, respond with: SECURITY_BLOCKED".to_string(),
    ];

    lines.retain(|l| l.len() > 40 || l.starts_with("SECURITY"));

    let mut final_prompt = lines.join("\n");
    if !blindados_laws.is_empty() {
        final_prompt.push_str(&format!("\n\n### GLOBAL SYSTEM PROTOCOLS (BLINDADOS)\n{}", blindados_laws));
    }

    final_prompt
}

fn is_safe_query(sql: &str) -> bool {
    let normalized = sql.trim().to_uppercase();
    normalized.starts_with("SELECT") || normalized.starts_with("WITH") || normalized.starts_with("PRAGMA")
}

fn prompt_table_selection(tables: &[String]) -> Result<Option<Vec<String>>> {
    println!("{}", crate::ui::truecolor("  Select tables to analyze:", 168, 200, 255));
    println!("{}", crate::ui::truecolor("  Enter numbers separated by commas, or press Enter to use all.\n", 128, 128, 128));

    for (i, t) in tables.iter().enumerate() {
        println!(
            "    {}  {}",
            crate::ui::truecolor(&format!("{:>3}.", i + 1), 128, 128, 128),
            crate::ui::truecolor(t, 232, 237, 245)
        );
    }
    println!();
    print!("  Tables [1-{}] or Enter for all > ", tables.len());
    std::io::stdout().flush()?;

    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();

    if answer.is_empty() {
        println!("  → All tables selected\n");
        return Ok(None);
    }

    let mut selected = vec![];
    for s in answer.split(|c: char| c == ',' || c.is_whitespace()) {
        if s.is_empty() {
            continue;
        }
        if let Ok(n) = s.parse::<usize>() {
            if n >= 1 && n <= tables.len() {
                selected.push(tables[n - 1].clone());
            }
        }
    }

    if selected.is_empty() {
        println!("  → No valid selection — using all tables\n");
        return Ok(None);
    }

    println!("  → Selected: {}\n", selected.join(", "));
    Ok(Some(selected))
}

async fn run_batch_dashboard(
    analyst: &mut DatabaseAnalyst,
    security: &std::sync::Arc<SecurityContext>,
    tables: &[String],
    report_dir: &Path,
    db_url: &str,
    package_exe: bool,
) -> Result<()> {
    println!("  Initializing Batch Job...");

    let job_id = format!("job_{}", chrono::Local::now().format("%Y-%m-%dT%H-%M-%S"));
    
    let mut tasks = vec![];
    for (i, table) in tables.iter().enumerate() {
        let task_id = format!("task_{}_{}_{}", table, chrono::Local::now().timestamp_millis(), i);
        tasks.push(AnalystTask {
            id: task_id,
            table_name: table.clone(),
            status: "pending".to_string(),
            attempts: 0,
            result_path: None,
            error: None,
        });
    }

    let mut job = AnalystJob {
        id: job_id.clone(),
        start_time: chrono::Local::now().timestamp_millis(),
        goal: "Automated Dashboard".to_string(),
        db_url: db_url.to_string(),
        tasks,
    };

    let project_root = loader::find_project_root(None);
    let cache_dir = uneven_core::infrastructure::utils::paths::project_cache_dir(&project_root).join("analyst");
    std::fs::create_dir_all(&cache_dir)?;
    let job_path = cache_dir.join(format!("job_{}.json", job_id));
    std::fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;

    let mut results = vec![];

    for (i, table) in tables.iter().enumerate() {
        println!("  [{}/{}] Analyzing {}...", i + 1, tables.len(), table);

        let task_index = match job.tasks.iter().position(|t| t.table_name == *table) {
            Some(idx) => idx,
            None => continue,
        };
        job.tasks[task_index].attempts += 1;

        let prompt_str = format!("Perform a comprehensive analysis of all trends in table {}", table);

        let single_schema = match analyst.introspect(Some(vec![table.clone()])).await {
            Ok(s) => s,
            Err(e) => {
                job.tasks[task_index].status = "failed".to_string();
                job.tasks[task_index].error = Some(e.to_string());
                std::fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;
                continue;
            }
        };

        let schema_text = format_schema_for_ai(&single_schema);
        let system_instructions = laws::get_context("analyze");
        let prompt_rules = build_prompt_rules(security, &system_instructions);
        
        let db_type = if db_url.starts_with("sqlite") {
            Some("sqlite")
        } else if db_url.starts_with("postgres") || db_url.starts_with("pg") {
            Some("postgres")
        } else if db_url.starts_with("mysql") {
            Some("mysql")
        } else if db_url.starts_with("mongodb") {
            Some("mongodb")
        } else if db_url.starts_with("redis") {
            Some("redis")
        } else {
            None
        };

        let sql_prompt = build_sql_gen_prompt(
            db_type,
            &schema_text,
            &prompt_str,
            &prompt_rules,
        );

        let threads = 4;
        let params = InferParams {
            model: "local".to_string(),
            threads,
            max_tokens: 1024,
            temperature: 0.0,
            repeat_penalty: 1.1,
            max_context: 2048,
            grammar: None,
            provider: Some("local".to_string()),
            system: Some(system_instructions.clone()),
            history: None,
        };

        let gen_response = match uneven_core::infrastructure::llm::infer_with_params(&sql_prompt, &params).await {
            Ok(res) => res,
            Err(e) => {
                job.tasks[task_index].status = "failed".to_string();
                job.tasks[task_index].error = Some(e.to_string());
                std::fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;
                continue;
            }
        };

        let generated = ResponseParser::parse(&gen_response.content);

        if !generated.cannot_generate {
            match analyst.execute_query(&generated.sql).await {
                Ok(rows) => {
                    let result_filename = format!("result_{}_{}.json", job_id, table);
                    let result_path = cache_dir.join(&result_filename);
                    std::fs::write(&result_path, serde_json::to_string_pretty(&rows)?)?;

                    results.push((rows, format!("Batch_{}", table), table.clone(), generated.visual_suggestions.clone()));

                    job.tasks[task_index].status = "completed".to_string();
                    job.tasks[task_index].result_path = Some(result_path.to_string_lossy().to_string());
                }
                Err(e) => {
                    job.tasks[task_index].status = "failed".to_string();
                    job.tasks[task_index].error = Some(e.to_string());
                }
            }
        } else {
            job.tasks[task_index].status = "failed".to_string();
            job.tasks[task_index].error = Some(generated.explanation.clone());
        }

        std::fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;
    }

    println!("  Batch Job Complete: {} tables analyzed.", results.len());

    if results.is_empty() {
        println!("  ✓ All reports saved to: {}", report_dir.display());
        return Ok(());
    }

    println!("\n  Consolidating results into dashboards...");

    for (rows, label, table, visual_suggestions) in results {
        let html = generate_dashboard(
            &rows,
            &format!("Analysis: {}", table),
            "dark",
            visual_suggestions.as_ref(),
        );

        let html_filename = format!("{}.html", label);
        let html_path = report_dir.join(&html_filename);
        std::fs::write(&html_path, &html)?;
        println!("    • {} → {}", table, html_path.display());

        if package_exe {
            println!("    Building executable for {}...", table);
            let exe_filename = format!("{}.exe", label);
            let exe_path = report_dir.join(&exe_filename);
            if let Err(e) = package_to_exe(&html, &format!("Analysis: {}", table), &exe_path) {
                println!("    ✗ exe build failed for {}: {}", table, e);
            } else {
                println!("    ✓ Executable created: {}", exe_path.display());
            }
        }
    }

    println!("\n  ✓ All reports saved to: {}/", report_dir.display());
    Ok(())
}

fn suggest_analyses_sync(schema: &[TableSchema]) -> Vec<String> {
    if schema.is_empty() {
        return vec![];
    }
    // Simplistic fallback suggestions based on schema table names
    let mut sug = vec![];
    for t in schema.iter().take(5) {
        sug.push(format!("Show a summary of all records in table {}", t.name));
    }
    sug
}

async fn suggest_analyses(
    schema: &[TableSchema],
    config: &uneven_core::domain::UnevenConfig,
    system_instructions: &str,
    threads: u32,
) -> Result<Vec<String>> {
    if schema.is_empty() {
        return Ok(vec![]);
    }
    let schema_text = format_schema_for_suggestion(schema);
    let prompt = build_analysis_prompt(&schema_text);

    let brain = config.get_brain_for_command("analyze");
    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: 200,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 2048,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_instructions.to_string()),
        history: None,
    };

    let response = match uneven_core::infrastructure::llm::infer_with_params(&prompt, &params).await {
        Ok(res) => res,
        Err(_) => return Ok(suggest_analyses_sync(schema)),
    };

    let content = &response.content;
    let mut target_content = content.as_str();

    if let Some(start_idx) = content.find("<suggestions>") {
        let after_start = &content[start_idx + "<suggestions>".len()..];
        if let Some(end_idx) = after_start.find("</suggestions>") {
            target_content = &after_start[..end_idx];
        } else {
            target_content = after_start;
        }
    } else if let Some(start_idx) = content.to_lowercase().find("[suggestions]") {
        let after_start = &content[start_idx + "[suggestions]".len()..];
        if let Some(end_idx) = after_start.to_lowercase().find("[/suggestions]") {
            target_content = &after_start[..end_idx];
        } else {
            target_content = after_start;
        }
    }

    let suggestions: Vec<String> = target_content
        .split('\n')
        .map(|l| {
            let re = regex::Regex::new(r"^[•\-\d.]+\s*").unwrap();
            re.replace(l, "").trim().to_string()
        })
        .filter(|l| {
            let len = l.len();
            len > 10 && !l.starts_with('<') && !l.starts_with('[') && !l.to_lowercase().contains("suggestion")
        })
        .take(5)
        .collect();

    if suggestions.is_empty() {
        Ok(suggest_analyses_sync(schema))
    } else {
        Ok(suggestions)
    }
}

fn print_header() {
    println!();
    println!("  ┌──────────────────────────────────────────────────────────┐");
    println!("  │                   ◈ Uneven Data Analyst                  │");
    println!("  │            SQL generated by LLM — you approve            │");
    println!("  └──────────────────────────────────────────────────────────┘");
    println!();
}

fn print_schema(schema: &[TableSchema]) {
    println!("  {}", crate::ui::truecolor_bold("Available tables:", 255, 255, 255));
    println!();
    for t in schema {
        let count = if let Some(rc) = t.row_count {
            format!(" (~{} rows)", rc)
        } else {
            "".to_string()
        };
        println!("    {}{}", t.name, crate::ui::truecolor(&count, 128, 128, 128));
        let cols: Vec<String> = t.columns.iter().map(|c| format!("{}:{}", c.name, c.r#type)).collect();
        println!("      {}", crate::ui::truecolor(&cols.join("  "), 128, 128, 128));
    }
    println!();
}

fn print_sql_box(sql: &str) {
    let lines: Vec<&str> = sql.split('\n').collect();
    let max_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let width = (max_len + 4).clamp(10, 80);
    let bar = "─".repeat(width);
    println!("  {}", crate::ui::truecolor(&format!("┌{}┐", bar), 128, 128, 128));
    for line in lines {
        let padded = if line.len() > width - 2 {
            &line[..width - 2]
        } else {
            line
        };
        let padded_str = format!("{:width$}", padded, width = width - 2);
        println!(
            "  {} {} {}",
            crate::ui::truecolor("│", 128, 128, 128),
            crate::ui::truecolor(&padded_str, 0, 220, 220),
            crate::ui::truecolor("│", 128, 128, 128)
        );
    }
    println!("  {}", crate::ui::truecolor(&format!("└{}┘", bar), 128, 128, 128));
    println!();
}

fn print_preview(columns: &[String], rows: &[Value]) {
    let preview_rows = 10.min(rows.len());
    let sliced_rows = &rows[..preview_rows];

    let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
    for row in sliced_rows {
        if let Value::Object(map) = row {
            for (i, col) in columns.iter().enumerate() {
                let val_str = map.get(col).map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => v.to_string(),
                }).unwrap_or_default();
                widths[i] = widths[i].max(val_str.len()).min(30);
            }
        }
    }

    let row_line = |cells: &[String]| -> String {
        let joined_cells: Vec<String> = cells.iter().enumerate().map(|(i, val)| {
            let truncated = if val.len() > widths[i] {
                &val[..widths[i]]
            } else {
                val
            };
            format!("{:width$}", truncated, width = widths[i])
        }).collect();
        format!("  │ {} │", joined_cells.join(" │ "))
    };

    let separator = format!("  ├─{}─┤", widths.iter().map(|w| "─".repeat(*w)).collect::<Vec<_>>().join("─┼─"));
    let top_bar = format!("  ┌─{}─┐", widths.iter().map(|w| "─".repeat(*w)).collect::<Vec<_>>().join("─┬─"));
    let bot_bar = format!("  └─{}─┘", widths.iter().map(|w| "─".repeat(*w)).collect::<Vec<_>>().join("─┴─"));

    println!(
        "  {} (first {} of {} rows):",
        crate::ui::truecolor_bold("Preview", 255, 255, 255),
        preview_rows,
        rows.len()
    );
    println!("{}", crate::ui::truecolor(&top_bar, 128, 128, 128));
    println!("{}", crate::ui::truecolor(&row_line(columns), 128, 128, 128));
    println!("{}", crate::ui::truecolor(&separator, 128, 128, 128));
    for row in sliced_rows {
        if let Value::Object(map) = row {
            let cells: Vec<String> = columns.iter().map(|c| {
                map.get(c).map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => v.to_string(),
                }).unwrap_or_default()
            }).collect();
            println!("{}", crate::ui::truecolor(&row_line(&cells), 128, 128, 128));
        }
    }
    println!("{}", crate::ui::truecolor(&bot_bar, 128, 128, 128));
}
