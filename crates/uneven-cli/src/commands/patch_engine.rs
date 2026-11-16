use crate::ui::truecolor;
use anyhow::Result;
use indicatif::ProgressBar;
use std::path::Path;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::llm::{
    InferParams, infer_with_params_streaming, init as init_llm,
};
use uneven_core::infrastructure::security::SafetyGuard;

#[allow(unused_imports)]
pub use uneven_core::infrastructure::patch::{
    apply_file_actions, clean_code_fence, file_object, is_safe_filename, is_safe_path_internal,
    normalize_path, parse_markdown_patches, FileObject, PatchActionKind, PatchActionResult,
    PatchObj, ResponsePayload,
};

pub fn build_prompt(user_request: &str, session_id: &str) -> String {
    format!(
        r#"
CRITICAL: Code and inline comments must be strictly in English.

For this session, the unique ID is: {session_id}.
Respond using structured Markdown code blocks (```).
Do NOT return JSON. Format each file action as follows:

### Creating a file:
### File: <relative_path>
**Action**: `create`

```<language>
<full content of the file>
```

### Editing an existing file:
### File: <relative_path>
**Action**: `edit`

```<language>
<<<<<<< SEARCH
<exact lines to find and replace>
=======
<new replacement code, or empty to remove>
>>>>>>> REPLACE
```

### Appending to a file:
### File: <relative_path>
**Action**: `append` (anchor: `up` | `down`)

```<language>
<<<<<<< ANCHOR
<target line to anchor, or empty for top/bottom>
=======
<content to insert>
>>>>>>> REPLACE
```

### Deleting a file:
### File: <relative_path>
**Action**: `delete`

### Renaming a file:
### File: <old_relative_path>
**Action**: `rename` -> `<new_relative_path>`

User request: {user_request}
"#
    )
}

/// Apply the file actions to disk and print CLI progress
pub fn file_action(root: &Path, file_obj: &FileObject) -> Vec<PatchActionResult> {
    let results = apply_file_actions(root, file_obj);
    for res in &results {
        match &res.kind {
            PatchActionKind::Created(p) => println!("  ✓ created file: {}", p),
            PatchActionKind::Edited(p) => println!("  ✓ edited file: {}", p),
            PatchActionKind::Renamed { from, to } => {
                println!("  ✓ renamed file: {} -> {}", from, to)
            }
            PatchActionKind::Deleted(p) => println!("  ✓ deleted file: {}", p),
            PatchActionKind::Appended { path, anchor } => {
                println!("  ✓ appended to file (anchor: {}): {}", anchor, path)
            }
            PatchActionKind::Failed { path, reason } => {
                println!("  ⚠ {} failed: {}", path, reason)
            }
            PatchActionKind::SkippedUnsafe(p) => {
                println!("  ⚠ skipped unsafe path: {}", p)
            }
        }
    }
    if let Some(ref summary) = file_obj.summary {
        let trimmed = summary.trim();
        if !trimmed.is_empty() {
            if !file_obj.patch.is_empty() {
                println!();
            }
            println!("  › Summary: {}", trimmed);
            println!();
        }
    }
    results
}

#[allow(dead_code)]
pub async fn run(
    task: String,
    provider_opt: Option<String>,
    model_opt: Option<String>,
    session_id: Option<String>,
) -> Result<()> {
    // 1. Safety Guard Validation (Shield)
    let safety = SafetyGuard::new();
    if let Err(err) = safety.validate_intent(&task) {
        println!(
            "\n{} {}",
            crate::ui::truecolor_bold("⚠️ [SECURITY POLICY]:", 255, 107, 53),
            err
        );
        return Ok(());
    }

    let session_id =
        session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()[..6].to_uppercase());

    // 2. Load config
    let root = loader::find_project_root(None);

    // 2.1 Check if user confirms / approves applying a pending patch
    if uneven_core::application::shell_classifier::is_approval_intent(&task)
        || task.trim().is_empty()
        || task.trim().to_lowercase() == "apply"
    {
        let pending = uneven_core::infrastructure::llm::history_manager::get_pending_patch_from_disk(&session_id, &root)
            .or_else(|| uneven_core::infrastructure::llm::history_manager::get_pending_patch_from_disk("default", &root));

        if let Some(file_obj) = pending
            && !file_obj.patch.is_empty()
        {
            println!(
                "{} {}",
                crate::ui::truecolor("  › ", 168, 200, 255),
                crate::ui::truecolor("Applying pending patch actions to disk:", 107, 122, 150)
            );
            file_action(&root, &file_obj);
            println!();
            let _ = uneven_core::infrastructure::llm::history_manager::clear_pending_patch_from_disk(&session_id, &root);
            let _ = uneven_core::infrastructure::llm::history_manager::clear_pending_patch_from_disk("default", &root);
            return Ok(());
        }
    }

    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("askf");
    if let Some(ref p) = provider_opt {
        brain.provider = p.clone();
    }
    if let Some(ref m) = model_opt {
        brain.model = m.clone();
    }

    // 3. Assemble Context
    let context = super::context_assembler::ContextAssembler::assemble(
        &task,
        &config,
        &brain,
        &root,
        None,
        &[],
    )
    .await
    .unwrap_or_else(|_| "(No relevant context found)\n\n".to_string());

    // 4. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    init_llm(&brain.provider, &brain.model, threads).await?;

    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let user_content = format!("{}\nUser request: {}", context, task);
    let prompt = build_prompt(&user_content, &session_id);

    // Use the max_context defined in the brain configuration, defaulting to a provider-aware fallback
    let max_context = if brain.provider == "local" {
        4096 // Safe default VRAM budget for local inference
    } else {
        1000000 // Modern default for cloud APIs (OpenAI, Gemini, Anthropic, etc.)
    };
    let params = InferParams {
        model: brain.model.clone(),
        threads,
        max_tokens: brain.max_tokens,
        temperature: brain.temperature as f64,
        repeat_penalty: 1.1,
        max_context,
        grammar: None,
        provider: Some(brain.provider.clone()),
        system: Some(
            uneven_core::infrastructure::security::laws::get_context("dde")
                .replace("{session_id}", &session_id),
        ),
        history: None,
    };

    let res = match infer_with_params_streaming(&prompt, &params, move |_token| {}).await {
        Ok(r) => {
            spinner.finish_and_clear();
            r
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(e.into());
        }
    };

    // Response Censorship Check
    let answer = safety.sanitize_response(&res.content);

    // 4. Convert the AI's JSON response to a `FileObject` and apply the file actions
    if let Some(file_obj) = file_object(&answer, &session_id) {
        if !file_obj.patch.is_empty() {
            println!(
                "{} {}",
                truecolor("  › ", 168, 200, 255),
                truecolor("Applying file actions:", 107, 122, 150)
            );
            file_action(&root, &file_obj);
            println!();
        } else {
            println!(
                "{}",
                truecolor("  › The LLM returned an empty patch set.", 107, 122, 150)
            );
        }
    } else {
        println!(
            "{}",
            truecolor(
                "  › The LLM did not declare any valid JSON files.",
                107,
                122,
                150
            )
        );
        println!(
            "{}",
            truecolor(
                "  › Try: \"create a file called index.html with a login form\"",
                107,
                122,
                150
            )
        );
        println!();
    }

    Ok(())
}

#[allow(dead_code)]
fn format_duration(d: std::time::Duration) -> String {
    let secs_f = d.as_secs_f64();
    if secs_f < 1.0 {
        let ms = (secs_f * 1000.0).round() as u64;
        format!("{}ms", ms)
    } else if secs_f < 60.0 {
        let secs = secs_f.round() as u64;
        format!("{}s", secs)
    } else {
        let total_secs = secs_f.round() as u64;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs == 0 {
            format!("{}m", mins)
        } else {
            format!("{}m {}s", mins, secs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    #[test]
    fn test_file_object_create() {
        let response = r#"{
            "file": {
                "session_id": "ABC123",
                "patch": [
                    {
                        "path": "src/index.js",
                        "mode": "create",
                        "content": "console.log('hello');\nconst x = 42;"
                    }
                ]
            }
        }"#;

        let parsed = file_object(response, "ABC123").expect("Should parse valid JSON");
        assert_eq!(parsed.session_id, "ABC123");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].path, "src/index.js");
        assert_eq!(parsed.patch[0].mode, "create");
        assert!(parsed.patch[0].content.contains("console.log('hello');"));
    }

    #[test]
    fn test_file_object_edit() {
        let response = r#"{
            "file": {
                "session_id": "XYZ789",
                "patch": [
                    {
                        "path": "src/main.rs",
                        "mode": "edit",
                        "search": "old_code();",
                        "content": "new_code();"
                    }
                ]
            }
        }"#;

        let parsed = file_object(response, "XYZ789").expect("Should parse valid JSON");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].mode, "edit");
        assert_eq!(parsed.patch[0].search, "old_code();");
        assert_eq!(parsed.patch[0].content, "new_code();");
    }

    #[test]
    fn test_file_object_append_up_down() {
        let response = r#"{
            "file": {
                "session_id": "INS123",
                "patch": [
                    {
                        "path": "src/main.rs",
                        "mode": "append",
                        "anchor": "up",
                        "search": "fn main() { }",
                        "content": "// Initialize config"
                    },
                    {
                        "path": "src/main.rs",
                        "mode": "append",
                        "anchor": "down",
                        "search": "fn main() { }",
                        "content": "println!(\"Hello!\");"
                    }
                ]
            }
        }"#;

        let parsed = file_object(response, "INS123").expect("Should parse valid JSON");
        assert_eq!(parsed.patch.len(), 2);

        assert_eq!(parsed.patch[0].anchor, "up");
        assert_eq!(parsed.patch[0].content, "// Initialize config");

        assert_eq!(parsed.patch[1].anchor, "down");
        assert_eq!(parsed.patch[1].content, "println!(\"Hello!\");");
    }

    #[test]
    fn test_file_object_delete() {
        let response = r#"{
            "file": {
                "session_id": "DEL001",
                "patch": [
                    {
                        "path": "src/old.rs",
                        "mode": "delete"
                    }
                ]
            }
        }"#;

        let parsed = file_object(response, "DEL001").expect("Should parse valid JSON");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].path, "src/old.rs");
        assert_eq!(parsed.patch[0].mode, "delete");
    }

    #[test]
    fn test_file_object_rename() {
        let response = r#"{
            "file": {
                "session_id": "REN001",
                "patch": [
                    {
                        "path": "src/old_name.rs",
                        "mode": "rename",
                        "rename_to": "src/new_name.rs"
                    }
                ]
            }
        }"#;

        let parsed = file_object(response, "REN001").expect("Should parse valid JSON");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].path, "src/old_name.rs");
        assert_eq!(parsed.patch[0].rename_to, "src/new_name.rs");
    }

    #[test]
    fn test_file_object_fallback_session_id() {
        let response = r#"{
            "file": {
                "session_id": "",
                "patch": [
                    {
                        "path": "src/main.rs",
                        "mode": "delete"
                    }
                ]
            }
        }"#;

        let parsed = file_object(response, "FALLBACK123").expect("Should parse valid JSON");
        assert_eq!(parsed.session_id, "FALLBACK123");
    }

    #[test]
    fn test_file_object_invalid_json() {
        let response = "This is plain text response with no JSON";
        let parsed = file_object(response, "ABC123");
        assert!(parsed.is_none());
    }

    #[test]
    fn test_clean_code_fence() {
        assert_eq!(
            clean_code_fence("```rust\nfn main() {}\n```"),
            "fn main() {}"
        );
        assert_eq!(
            clean_code_fence("```javascript\nconsole.log('test');\n```"),
            "console.log('test');"
        );
        assert_eq!(
            clean_code_fence("```\nlet x = 42;\n```"),
            "let x = 42;"
        );
        assert_eq!(clean_code_fence("`const y = 10;`"), "const y = 10;");
        assert_eq!(clean_code_fence("raw_code();"), "raw_code();");
    }

    #[test]
    fn test_file_object_markdown_create() {
        let response = r#"
Here is the code to create the component:

### File: `src/components/card.rs`
**Action**: `create`

```rust
pub struct Card {
    pub title: String,
}
```

## Summary
Created Card component
"#;
        let parsed = file_object(response, "MD_001").expect("Should parse Markdown create action");
        assert_eq!(parsed.session_id, "MD_001");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].path, "src/components/card.rs");
        assert_eq!(parsed.patch[0].mode, "create");
        assert!(parsed.patch[0].content.contains("pub struct Card"));
        assert!(!parsed.patch[0].content.contains("```"));
    }

    #[test]
    fn test_file_object_markdown_edit_search_replace() {
        let response = r#"
### File: `src/main.rs`
**Action**: `edit`

```rust
<<<<<<< SEARCH
fn old_calc() -> i32 {
    0
}
=======
fn old_calc() -> i32 {
    42
}
>>>>>>> REPLACE
```
"#;
        let parsed = file_object(response, "MD_002").expect("Should parse Markdown edit action");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].path, "src/main.rs");
        assert_eq!(parsed.patch[0].mode, "edit");
        assert_eq!(parsed.patch[0].search, "fn old_calc() -> i32 {\n    0\n}");
        assert_eq!(parsed.patch[0].content, "fn old_calc() -> i32 {\n    42\n}");
    }

    #[test]
    fn test_file_object_markdown_append() {
        let response = r#"
### File: `src/lib.rs`
**Action**: `append` (anchor: `down`)

```rust
<<<<<<< ANCHOR
pub mod parser;
=======
pub mod lexer;
>>>>>>> REPLACE
```
"#;
        let parsed = file_object(response, "MD_003").expect("Should parse Markdown append action");
        assert_eq!(parsed.patch.len(), 1);
        assert_eq!(parsed.patch[0].path, "src/lib.rs");
        assert_eq!(parsed.patch[0].mode, "append");
        assert_eq!(parsed.patch[0].anchor, "down");
        assert_eq!(parsed.patch[0].search, "pub mod parser;");
        assert_eq!(parsed.patch[0].content, "pub mod lexer;");
    }

    #[test]
    fn test_file_object_markdown_delete_and_rename() {
        let response = r#"
### File: `src/old.rs`
**Action**: `delete`

### File: `src/v1.rs`
**Action**: `rename` -> `src/v2.rs`
"#;
        let parsed = file_object(response, "MD_004").expect("Should parse Markdown delete and rename");
        assert_eq!(parsed.patch.len(), 2);
        assert_eq!(parsed.patch[0].path, "src/old.rs");
        assert_eq!(parsed.patch[0].mode, "delete");

        assert_eq!(parsed.patch[1].path, "src/v1.rs");
        assert_eq!(parsed.patch[1].mode, "rename");
        assert_eq!(parsed.patch[1].rename_to, "src/v2.rs");
    }

    #[test]
    fn test_is_safe_filename() {
        assert!(is_safe_filename("src/app.ts"));
        assert!(is_safe_filename("index.js"));
        assert!(is_safe_filename("Cargo.toml"));

        assert!(!is_safe_filename("/etc/passwd"));
        assert!(!is_safe_filename("../outside.ts"));
        assert!(!is_safe_filename("src/../../outside.ts"));
        assert!(!is_safe_filename(r"src\..\..\outside.ts"));
    }

    #[test]
    fn test_is_safe_path_internal() {
        let root = Path::new("/home/user/project");
        let external = Path::new("/home/user/other");

        assert!(is_safe_path_internal("src/main.rs", root, Some(external)));
        assert!(is_safe_path_internal(
            "/home/user/project/src/main.rs",
            root,
            Some(external)
        ));

        assert!(is_safe_path_internal(
            "../other/src/lib.rs",
            root,
            Some(external)
        ));
        assert!(is_safe_path_internal(
            "/home/user/other/src/lib.rs",
            root,
            Some(external)
        ));

        assert!(!is_safe_path_internal("/etc/passwd", root, Some(external)));
        assert!(!is_safe_path_internal(
            "../outside/src/lib.rs",
            root,
            Some(external)
        ));
        assert!(!is_safe_path_internal(
            "../../etc/passwd",
            root,
            Some(external)
        ));
    }
}
