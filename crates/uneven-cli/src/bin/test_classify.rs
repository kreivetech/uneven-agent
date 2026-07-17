use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::llm::{InferParams, init as init_llm, infer_with_params};
use uneven_core::infrastructure::knowledge::incremental_index::IncrementalIndex;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;
    let brain = config.get_brain_for_command("ask");

    init_llm(&brain.provider, &brain.model, 4).await?;

    let inc_idx = IncrementalIndex::new(&root);
    let files: Vec<String> = inc_idx.state.files.keys()
        .filter_map(|abs_path| {
            Path::new(abs_path).strip_prefix(&root).ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect();

    println!("Total files in index: {}", files.len());
    if files.is_empty() {
        println!("No files in index!");
        return Ok(());
    }

    let files_str = files.join("\n");
    let system_prompt = "You are a software developer assistant. Your task is to analyze a list of files in the project and identify which files are relevant to answer the user's question or request.\n\n\
    Instructions:\n\
    - Return ONLY a JSON array of strings containing the relative file paths of the relevant files.\n\
    - Example: [\"src/main.rs\", \"Cargo.toml\"]\n\
    - Do NOT include markdown styling, code blocks (```json), or any explanation. Output only raw JSON.\n\
    - Select at most 5-10 files. Be precise and select only files that are directly related to the user's query.\n\
    - If no files are relevant, return [].";

    let params = InferParams {
        model: brain.model.clone(),
        threads: 4,
        max_tokens: 2000,
        temperature: 0.0,
        repeat_penalty: 1.1,
        max_context: 8192,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(system_prompt.to_string()),
        history: None,
    };

    let question = "How does the scheduler balance tasks across CPU cores?";
    let user_msg = format!("Files:\n{}\n\nQuestion: {}", files_str, question);

    println!("Sending classification request to Gemini...");
    let resp = infer_with_params(&user_msg, &params).await?;
    println!("Response content:\n{}", resp.content);

    Ok(())
}
