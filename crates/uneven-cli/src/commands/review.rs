use anyhow::Result;
use console::style;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::process::Command as SysCommand;

use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::llm::{
    self, InferParams,
};

const MAX_DIFF_CHARS: usize = 32_000;

#[derive(Debug)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

pub fn parse_diff_stats(diff: &str) -> DiffStats {
    let mut files = HashSet::new();
    let mut lines_added = 0;
    let mut lines_removed = 0;

    for line in diff.lines() {
        if let Some(stripped) = line.strip_prefix("+++ b/") {
            files.insert(stripped.to_string());
        } else if line.starts_with('+') && !line.starts_with("+++") {
            lines_added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            lines_removed += 1;
        }
    }

    DiffStats {
        files_changed: files.len(),
        lines_added,
        lines_removed,
    }
}

pub fn build_review_prompt(diff: &str, context: &str) -> String {
    let truncated = if diff.len() > MAX_DIFF_CHARS {
        format!(
            "{}\n\n[diff truncated — showing first {} chars]",
            &diff[..MAX_DIFF_CHARS],
            MAX_DIFF_CHARS
        )
    } else {
        diff.to_string()
    };

    format!(
        r#"You are a senior software engineer performing a thorough code review.
Analyze the following git diff and provide structured, actionable feedback.

Format your response exactly as:

## Summary
[1–3 sentences describing what the diff does overall]

## Findings
[For each issue, use this format:]
**[SEVERITY] category — path/to/file[:line]**
What the problem is and why it matters.
> Suggestion: concrete fix or improvement

Severities: CRITICAL · HIGH · MEDIUM · LOW · INFO
Categories: bug · security · performance · style · maintainability

If there are no issues write: ✓ No issues found.

## Verdict
[APPROVE | REQUEST CHANGES | NEEDS DISCUSSION] — one sentence rationale

---
{}
## Diff to Review

```diff
{}
```"#,
        if context.is_empty() {
            "".to_string()
        } else {
            format!("## Context\n{}\n\n---\n", context)
        },
        truncated
    )
}

pub fn get_git_diff(
    from: Option<String>,
    commit: Option<String>,
    staged: bool,
    base: Option<String>,
    paths: Vec<String>,
) -> Result<(String, String, DiffStats)> {
    let root = loader::find_project_root(None);

    // Validate Git Repository
    let is_git = SysCommand::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&root)
        .output()
        .map(|out| out.status.success() && root.join(".git").exists())
        .unwrap_or(false);

    if !is_git {
        return Err(anyhow::anyhow!("[UE-03000003] Not a git repository. Please initialize git to run a review."));
    }

    let mut git_args = vec!["diff".to_string(), "-U5".to_string()];
    let mut diff_label = "working tree vs HEAD".to_string();

    if let Some(ref c) = commit {
        git_args = vec!["show".to_string(), "--format=".to_string(), "-U5".to_string(), c.clone()];
        
        let info_out = SysCommand::new("git")
            .args(["log", "-1", "--format=%s", c])
            .current_dir(&root)
            .output()?;
        let subject = String::from_utf8_lossy(&info_out.stdout).trim().to_string();
        diff_label = format!("commit {} — {}", &c[..c.len().min(8)], subject);
    } else if staged {
        git_args.push("--staged".to_string());
        diff_label = "staged changes".to_string();
    } else if let Some(ref b) = base {
        git_args.push(format!("{}...HEAD", b));
        diff_label = format!("{}...HEAD", b);
    } else if let Some(ref f) = from {
        git_args.push(format!("{}..HEAD", f));
        diff_label = format!("{}..HEAD", f);
    } else {
        git_args.push("HEAD".to_string());
    }

    if !paths.is_empty() {
        git_args.push("--".to_string());
        for p in &paths {
            git_args.push(p.clone());
        }
    }

    let diff_out = SysCommand::new("git")
        .args(&git_args)
        .current_dir(&root)
        .output()?;

    let diff = String::from_utf8_lossy(&diff_out.stdout).into_owned();
    let stats = parse_diff_stats(&diff);

    Ok((diff, diff_label, stats))
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    from: Option<String>,
    commit: Option<String>,
    staged: bool,
    report: bool,
    context: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    base: Option<String>,
    paths: Vec<String>,
) -> Result<()> {
    // 1. License Check (Requires Pro)
    super::license::require_pro()?;

    let root = loader::find_project_root(None);
    let (diff, diff_label, stats) = get_git_diff(from, commit, staged, base, paths)?;

    if diff.trim().is_empty() {
        println!(
            "{}",
            style("  [UE-03000003] ⚠ No changes found in diff — nothing to review.").yellow()
        );
        return Ok(());
    }

    let truncated = diff.len() > MAX_DIFF_CHARS;

    println!(
        "{} {}  {} file{}, {} / {}",
        style("✓").green().bold(),
        style(format!("Diff loaded: {}", diff_label)).bold(),
        stats.files_changed,
        if stats.files_changed != 1 { "s" } else { "" },
        style(format!("+{}", stats.lines_added)).green(),
        style(format!("-{}", stats.lines_removed)).red()
    );

    if truncated {
        println!(
            "{}",
            style(format!(
                "  ⚠ Diff exceeds {} chars — truncated for review",
                MAX_DIFF_CHARS
            ))
            .yellow()
        );
    }

    // 4. Load Configuration
    let config = loader::load_config(&root)?;
    let mut brain = config.get_brain_for_command("review");
    if let Some(p) = provider {
        brain.provider = p;
    }
    if let Some(m) = model {
        brain.model = m;
    }

    // 5. Initialize LLM
    let threads = brain.local.as_ref().and_then(|l| l.threads).unwrap_or(4);
    llm::init(&brain.provider, &brain.model, threads).await?;

    println!();
    println!(
        "{}",
        style(format!(
            "  ┌─ Code Review  [{}]",
            brain_label(
                &brain.provider,
                &brain.model
            )
        ))
        .dim()
    );
    print!("  │\n  │  ");
    std::io::stdout().flush()?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let tx_clone = tx.clone();

    let prompt = build_review_prompt(&diff, context.as_deref().unwrap_or(""));
    let prompt_clone = prompt.clone();
    let brain_clone = brain.clone();
    let params = InferParams {
        model: brain_clone.model.clone(),
        threads,
        max_tokens: brain_clone.max_tokens,
        temperature: brain_clone.temperature as f64,
        repeat_penalty: 1.1,
        max_context: 4096,
        grammar: None,
        provider: Some(brain_clone.provider.clone()),
        system: Some(uneven_core::infrastructure::security::laws::get_context("review")),
        history: None,
    };

    let prompt_to_use = if brain_clone.provider == "local" {
        let system_instructions = uneven_core::infrastructure::security::laws::get_context("review");
        let chat_template = llm::inference::chat_template::detect_template(&brain_clone.model);
        llm::inference::chat_template::apply_template(&chat_template, &system_instructions, &[], &prompt_clone)
    } else {
        prompt_clone
    };

    let prompt_to_use_clone = prompt_to_use.clone();
    let inference_handle = tokio::spawn(async move {
        llm::infer_with_params_streaming(
            &prompt_to_use_clone,
            &params,
            move |token| {
                let _ = tx_clone.blocking_send(token);
            }
        ).await
    });

    drop(tx);

    let mut review_text = String::new();
    while let Some(token) = rx.recv().await {
        review_text.push_str(&token);
        let parts: Vec<&str> = token.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                print!("\n  │  ");
            }
            print!("{}", style(part).color256(231)); // crystal white tone
        }
        std::io::stdout().flush()?;
    }

    let _ = inference_handle.await?;

    println!();
    println!("{}", style("  └─ Done").dim());
    println!();

    // 6. Save Report
    if report {
        let reports_dir = root.join(".uneven").join("reports").join("reviews");
        fs::create_dir_all(&reports_dir)?;
        let date_str = chrono::Utc::now().to_rfc3339().replace(':', "-");
        let report_path = reports_dir.join(format!("review-{}.md", date_str));

        let header = format!(
            "# Code Review — {}\n**Date:** {}\n**Files:** {}  **+{}** / **-{}**\n**Model:** {}\n\n---\n\n",
            diff_label,
            chrono::Utc::now().to_rfc3339(),
            stats.files_changed,
            stats.lines_added,
            stats.lines_removed,
            brain_label(
                &brain.provider,
                &brain.model
            )
        );

        fs::write(&report_path, header + &review_text)?;
        println!(
            "  {}",
            style(format!("Report saved: {}", report_path.to_string_lossy())).cyan()
        );
        println!();
    }

    Ok(())
}

fn brain_label(provider: &str, model: &str) -> String {
    format!("{}/{}", provider, model)
}
