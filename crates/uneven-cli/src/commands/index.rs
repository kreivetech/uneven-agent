use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use uneven_core::infrastructure::config::loader;
use uneven_core::infrastructure::knowledge::incremental_index::IncrementalIndex;
use uneven_core::infrastructure::knowledge::scanner::ProjectScanner;
use uneven_core::infrastructure::utils::process_lock::ProcessLock;

const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024; // 2 MB
const HARD_SKIP_BYTES: u64 = 50 * 1024 * 1024; // 50 MB

pub async fn run(
    dir: Option<String>,
    full: bool,
    _dry_run: bool,
    skip_large: bool,
    max_file_size: Option<f64>,
    _yes: bool,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let root = loader::find_project_root(None);
    let mut config = loader::load_config(&root)?;

    let max_file_bytes = max_file_size
        .map(|mb| (mb * 1024.0 * 1024.0) as u64)
        .unwrap_or(DEFAULT_MAX_FILE_BYTES);

    let dirs = if let Some(ref d) = dir {
        vec![d.clone()]
    } else {
        config
            .knowledge
            .as_ref()
            .and_then(|k| k.dirs.clone())
            .unwrap_or_else(|| vec!["./".to_string()])
    };

    if let Some(ref mut k) = config.knowledge {
        k.dirs = Some(dirs.clone());
    }

    let mut inc_idx = IncrementalIndex::new(&root);
    if full {
        inc_idx.clear();
        inc_idx = IncrementalIndex::new(&root);
    }

    let mut lock = ProcessLock::new(&root);
    if let Some(existing) = lock.acquire("index")? {
        println!(
            "{}",
            style(format!(
                "  ✗ Command 'index' is locked by another active process (PID {}).",
                existing.pid
            ))
            .red()
        );
        std::process::exit(1);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")?,
    );
    spinner.set_message("Scanning project files…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let scanner = ProjectScanner::new(&root);
    let mut file_list = scanner.scan(dirs)?;

    // Add individual files from config.knowledge.files to be indexed
    if let Some(extra_files) = config.knowledge.as_ref().and_then(|k| k.files.clone()) {
        for f in extra_files {
            let normalized_f = f.replace('\\', "/");
            let abs_path = root.join(&normalized_f);
            if abs_path.exists() && abs_path.is_file() {
                file_list.push(normalized_f);
            }
        }
        file_list.sort();
        file_list.dedup();
    }

    // Save paths list cache immediately to the global cache directory
    let cache_dir = uneven_core::infrastructure::utils::paths::project_cache_dir(&root);
    let _ = fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join("file-list-cache.json");
    if let Ok(json_content) = serde_json::to_string(&file_list) {
        let _ = fs::write(&cache_path, json_content);
    }

    let _pruned = inc_idx.prune_deleted_files()?;

    let mut to_index = Vec::new();
    for fp in &file_list {
        let abs_path = root.join(fp);
        if !abs_path.exists() || abs_path.is_dir() {
            continue;
        }

        let size_bytes = fs::metadata(&abs_path).map(|m| m.len()).unwrap_or(0);
        if size_bytes >= HARD_SKIP_BYTES {
            continue;
        }
        if skip_large && size_bytes >= max_file_bytes {
            continue;
        }
        if is_binary_file(&abs_path) {
            continue;
        }

        let status = inc_idx.status(&abs_path.to_string_lossy());
        if status == "new" || status == "modified" {
            to_index.push((fp.clone(), abs_path, size_bytes));
        }
    }

    for (_, abs_path, size_bytes) in &to_index {
        if let Ok(raw_bytes) = fs::read(abs_path) {
            let file_hash = IncrementalIndex::hash_buffer(&raw_bytes);
            inc_idx.mark_indexed_with_hash(
                &abs_path.to_string_lossy(),
                file_hash,
                *size_bytes,
                Vec::new(),
            );
        }
    }

    // Index URLs from config (scrape and save as local file cache under global cache scraped_urls/)
    if let Some(ref k) = config.knowledge
        && let Some(ref urls) = k.urls
            && !urls.is_empty() {
                let scraper = uneven_core::infrastructure::knowledge::web_loader::WebScraper::new();
                let scrape_dir = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("scraped_urls");
                let _ = fs::create_dir_all(&scrape_dir);

                for url in urls {
                    if let Ok(doc) = scraper.fetch_url(url).await {
                        let safe_name = hash_path(url);
                        let path = scrape_dir.join(format!("{}.txt", safe_name));
                        if fs::write(&path, &doc.content).is_ok() {
                            let file_hash = IncrementalIndex::hash_buffer(doc.content.as_bytes());
                            inc_idx.mark_indexed_with_hash(
                                &path.to_string_lossy(),
                                file_hash,
                                doc.content.len() as u64,
                                Vec::new(),
                            );
                        }
                    }
                }
            }

    inc_idx.save()?;
    spinner.finish_and_clear();

    let duration = start_time.elapsed();
    let duration_secs = duration.as_secs_f64();
    println!(
        "  {} Knowledge base is up to date (Total files: {}, indexed/updated: {} files in {:.2}s)",
        crate::ui::truecolor("✓", 0, 255, 140),
        file_list.len(),
        to_index.len(),
        duration_secs
    );
    println!("  Your project is ready for querying\n");

    Ok(())
}

fn hash_path(file_path: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(file_path.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    hex[..12].to_string()
}

fn is_binary_file(path: &Path) -> bool {
    use std::io::Read;
    if let Ok(mut file) = fs::File::open(path) {
        let mut buffer = [0; 4096];
        if let Ok(n) = file.read(&mut buffer) {
            for &b in buffer.iter().take(n) {
                if b == 0 {
                    return true;
                }
            }
        }
    }
    false
}
