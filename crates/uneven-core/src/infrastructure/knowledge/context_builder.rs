use crate::infrastructure::config::loader;
use crate::infrastructure::knowledge::scanner::ProjectScanner;
use crate::infrastructure::knowledge::web_loader::{CachedSitemap, WebScraper};
use crate::infrastructure::patch::normalize_path;
use crate::infrastructure::utils::formatter::format_with_line_numbers;
use crate::infrastructure::utils::paths::project_cache_dir;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn load_and_format_file(root: &Path, rel_path: &str) -> Option<String> {
    let p = Path::new(rel_path);
    let abs_path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    };

    let norm_path = normalize_path(&abs_path);
    let config = loader::load_config(root).ok();
    let ext_project = config
        .as_ref()
        .and_then(|c| c.knowledge.as_ref())
        .and_then(|k| k.external_project.as_ref())
        .map(PathBuf::from);

    let mut is_safe = norm_path.starts_with(normalize_path(root));
    if !is_safe
        && let Some(ref ext) = ext_project
        && norm_path.starts_with(normalize_path(ext))
    {
        is_safe = true;
    }

    if !is_safe {
        return None;
    }

    if abs_path.exists() && abs_path.is_file() {
        if is_binary_file(&abs_path) {
            return None;
        }
        if let Ok(content) = std::fs::read_to_string(&abs_path) {
            let char_limit = 100_000;
            let (display_content, truncated) = if content.len() > char_limit {
                (format!("{}\n", &content[..char_limit]), true)
            } else {
                (content, false)
            };
            let numbered = format_with_line_numbers(&display_content);
            let mut formatted = format!("**File: {}**\n```\n{}", rel_path, numbered);
            if truncated {
                formatted.push_str("... (rest of file truncated to save context window)\n");
            }
            formatted.push_str("```\n\n");
            return Some(formatted);
        }
    }
    None
}

pub fn is_binary_file(path: &Path) -> bool {
    use std::io::Read;
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut buffer = [0; 1024];
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

pub fn parse_all_need_file_tags(text: &str) -> HashSet<String> {
    let mut files = HashSet::new();
    if let Ok(re) = regex::Regex::new(
        r#"(?i)NEED_FILE[\"\']?\s*[:=]?\s*[\"\']?[<\[\{\(]?([^\s\]\[\}\{\"\',;<>\(\)]+)"#,
    ) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let mut path = m.as_str().trim().to_string();
                while path.ends_with('.') || path.ends_with('?') || path.ends_with('!') {
                    path.pop();
                }
                files.insert(path);
            }
        }
    }
    files
}

pub fn sync_external_project(ext_project_path: &str) -> Option<Vec<String>> {
    let ext_path = Path::new(ext_project_path);
    if !ext_path.exists() || !ext_path.is_dir() {
        return None;
    }
    let canonical = ext_path.canonicalize().unwrap_or_else(|_| ext_path.to_path_buf());

    let scanner = ProjectScanner::new(&canonical);
    let mut files = scanner.scan(vec![".".to_string()]).unwrap_or_default();
    files.sort();
    files.dedup();

    let cache_dir = project_cache_dir(&canonical);
    let _ = std::fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join("file-list-cache.json");
    if let Ok(json_content) = serde_json::to_string(&files) {
        let _ = std::fs::write(&cache_path, json_content);
    }

    Some(files)
}

pub fn parse_all_web_search_tags(text: &str) -> HashSet<String> {
    let mut queries = HashSet::new();
    if let Ok(re) = regex::Regex::new(r#"(?i)\[WEB_SEARCH:\s*([^\]]+)\]"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let q = m.as_str().trim().to_string();
                if !q.is_empty() {
                    queries.insert(q);
                }
            }
        }
    }
    if let Ok(re) = regex::Regex::new(r#"(?i)WEB_SEARCH:\s*["']([^"']+)["']"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let q = m.as_str().trim().to_string();
                if !q.is_empty() {
                    queries.insert(q);
                }
            }
        }
    }
    queries
}

pub fn parse_all_web_url_tags(text: &str) -> HashSet<String> {
    let mut urls = HashSet::new();
    if let Ok(re) = regex::Regex::new(r#"(?i)\[WEB_URL:\s*([^\]]+)\]"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let u = m.as_str().trim().to_string();
                if !u.is_empty() {
                    urls.insert(u);
                }
            }
        }
    }
    if let Ok(re) = regex::Regex::new(r#"(?i)WEB_URL:\s*["']?([^\s\]}'"']+)["']?"#) {
        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let u = m.as_str().trim().to_string();
                if !u.is_empty() {
                    urls.insert(u);
                }
            }
        }
    }
    urls
}

pub async fn load_and_format_web_page(url: &str) -> Option<String> {
    let scraper = WebScraper::new();
    if let Ok(doc) = scraper.fetch_url(url).await {
        let mut formatted = String::new();
        formatted.push_str(&format!("\n### WEBSITE CONTENT FROM URL: {}\n", url));
        formatted.push_str(&format!("Title: {}\n\n", doc.title));
        formatted.push_str(&doc.content);
        formatted.push_str("\n\n");
        Some(formatted)
    } else {
        None
    }
}

pub async fn run_and_format_web_search(
    query: &str,
    project_root: &Path,
) -> Option<String> {
    let scraper = WebScraper::new();
    let mut search_res = Vec::new();
    if let Ok(results) = scraper.search_web(query, project_root).await {
        search_res.extend(results);
    }
    if let Ok(results) = scraper.search_wikipedia(query).await {
        search_res.extend(results);
    }
    if search_res.is_empty() {
        return None;
    }
    let mut formatted = String::new();
    formatted.push_str(&format!("\n### SEARCH RESULTS FOR QUERY: \"{}\"\n\n", query));
    for (i, res) in search_res.iter().enumerate() {
        formatted.push_str(&format!("{}. {}\n", i + 1, res.title));
        formatted.push_str(&format!("   URL: {}\n", res.url));
        formatted.push_str(&format!("   Snippet: {}\n\n", res.snippet));
    }

    // Scan local sitemaps cache directory
    let sitemaps_dir = project_cache_dir(project_root).join("web-sitemaps");
    if sitemaps_dir.exists()
        && let Ok(entries) = std::fs::read_dir(&sitemaps_dir)
    {
        let mut sitemap_hints = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            if let Ok(content) = std::fs::read_to_string(entry.path())
                && let Ok(cached) = serde_json::from_str::<CachedSitemap>(&content)
            {
                let query_words: Vec<&str> = query
                    .split_whitespace()
                    .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
                    .filter(|w| !w.is_empty())
                    .collect();
                let mut matches = false;
                for word in &query_words {
                    if cached.domain.to_lowercase().contains(&word.to_lowercase()) {
                        matches = true;
                        break;
                    }
                    for u in cached.urls.iter().take(50) {
                        if u.to_lowercase().contains(&word.to_lowercase()) {
                            matches = true;
                            break;
                        }
                    }
                }
                if matches {
                    sitemap_hints.push(cached);
                }
            }
        }
        if !sitemap_hints.is_empty() {
            formatted.push_str("### KNOWN CACHED PAGES (SITEMAPS) FOR DOMAINS MATCHING QUERY:\n");
            for hint in sitemap_hints {
                formatted.push_str(&format!(
                    "* Domain: {} (Sitemap URLs cached: {})\n",
                    hint.domain,
                    hint.urls.len()
                ));
                let query_words: Vec<&str> = query
                    .split_whitespace()
                    .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
                    .filter(|w| !w.is_empty())
                    .collect();
                let mut shown = 0;
                for url in &hint.urls {
                    let mut url_match = false;
                    for word in &query_words {
                        if url.to_lowercase().contains(&word.to_lowercase()) {
                            url_match = true;
                            break;
                        }
                    }
                    if url_match {
                        formatted.push_str(&format!("  - {}\n", url));
                        shown += 1;
                        if shown >= 5 {
                            break;
                        }
                    }
                }
            }
            formatted.push('\n');
        }
    }

    Some(formatted)
}
