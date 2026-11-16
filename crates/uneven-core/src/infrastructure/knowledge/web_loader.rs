//! Web content loader/scraper using reqwest and regex-based HTML cleaning
//! to convert web pages into readable text/markdown for indexing.

use std::collections::HashMap;
use std::time::Duration;
use reqwest::Client;
use regex::Regex;
use crate::UnevenError;
use once_cell::sync::Lazy;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct WebDocument {
    pub id: String,
    pub url: String,
    pub content: String,
    pub title: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone)]
pub struct WebScraper {
    client: Client,
    user_agent: String,
}

impl Default for WebScraper {
    fn default() -> Self {
        Self::new()
    }
}

impl WebScraper {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        WebScraper {
            client,
            user_agent: "Uneven Bot 1.0 (+https://github.com/uneven-agent/uneven-llm)".to_string(),
        }
    }

    pub async fn fetch_url(&self, url: &str) -> Result<WebDocument, UnevenError> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|e| UnevenError::KnowledgeError(format!("Invalid URL: {e}")))?;
        
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(UnevenError::KnowledgeError(format!(
                "[UE-06000002] Unsupported URL scheme \"{}\" — only http:// and https:// can be fetched. \
                Database URLs ({}//) belong in the \"db\" config section, not \"urls\".",
                parsed.scheme(),
                parsed.scheme()
            )));
        }

        let mut html = String::new();
        let mut content_type = String::new();
        let mut success = false;

        for retry in 0..=2 {
            match self.client.get(url)
                .header("User-Agent", &self.user_agent)
                .send()
                .await 
            {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        if retry == 2 {
                            return Err(UnevenError::KnowledgeError(format!(
                                "[UE-06000003] HTTP {}: {}", resp.status().as_u16(), resp.status().canonical_reason().unwrap_or("Unknown")
                            )));
                        }
                    } else {
                        content_type = resp.headers().get("content-type")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        html = resp.text().await
                            .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read response text: {e}")))?;
                        success = true;
                        break;
                    }
                }
                Err(e) => {
                    if retry == 2 {
                        return Err(UnevenError::KnowledgeError(format!(
                            "[UE-06000001] Web scrape failed for {url}: {e}"
                        )));
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        if !success {
            return Err(UnevenError::KnowledgeError(format!("Failed to fetch URL {url} after retries")));
        }

        let title = extract_title(&html);
        let content = extract_content(&html, &content_type);

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "web".to_string());
        metadata.insert("url".to_string(), url.to_string());
        metadata.insert("content_type".to_string(), content_type);
        metadata.insert("fetched_at".to_string(), chrono::Utc::now().to_rfc3339());
        metadata.insert("doc_type".to_string(), "web".to_string());

        Ok(WebDocument {
            id: url_to_id(url),
            url: url.to_string(),
            content,
            title,
            metadata,
        })
    }

    pub async fn fetch_multiple(&self, urls: &[String]) -> Vec<WebDocument> {
        let mut docs = Vec::new();
        let concurrency = 10;
        for chunk in urls.chunks(concurrency) {
            let mut futures = Vec::new();
            for url in chunk {
                futures.push(self.fetch_url(url));
            }
            let results = futures::future::join_all(futures).await;
            for (url, result) in chunk.iter().zip(results) {
                match result {
                    Ok(doc) => docs.push(doc),
                    Err(e) => tracing::warn!("Web: Error fetching {}: {}", url, e),
                }
            }
        }
        docs
    }
}

pub fn fnv1a32(str: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for &code_unit in &str.encode_utf16().collect::<Vec<u16>>() {
        h ^= code_unit as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

pub fn url_to_id(url_str: &str) -> String {
    if let Ok(url_obj) = reqwest::Url::parse(url_str) {
        let hostname = url_obj.host_str().unwrap_or("").replace('.', "_");
        let path = url_obj.path();
        let re_non_alphanumeric = Regex::new(r"[^a-zA-Z0-9]").unwrap();
        let safe_path = re_non_alphanumeric.replace_all(path, "_");
        let hash = fnv1a32(url_str);
        let id = format!("web_{}_{}_{}", hostname, safe_path, hash);
        if id.len() > 100 {
            id[..100].to_string()
        } else {
            id
        }
    } else {
        let re_non_alphanumeric = Regex::new(r"[^a-zA-Z0-9]").unwrap();
        let safe_url = re_non_alphanumeric.replace_all(url_str, "_");
        let id = format!("web_{}", safe_url);
        if id.len() > 100 {
            id[..100].to_string()
        } else {
            id
        }
    }
}

fn extract_title(html: &str) -> String {
    static RE_TITLE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<title[^>]*>([^<]+)</title>"#).unwrap());
    static RE_H1: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<h1[^>]*>([^<]+)</h1>"#).unwrap());

    if let Some(caps) = RE_TITLE.captures(html)
        && let Some(m) = caps.get(1) {
            let t = m.as_str().trim();
            if t.len() > 200 {
                return t[..200].to_string();
            } else {
                return t.to_string();
            }
        }

    if let Some(caps) = RE_H1.captures(html)
        && let Some(m) = caps.get(1) {
            let t = m.as_str().trim();
            if t.len() > 200 {
                return t[..200].to_string();
            } else {
                return t.to_string();
            }
        }

    "Untitled".to_string()
}

fn extract_content(html: &str, content_type: &str) -> String {
    if content_type.contains("application/json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(html)
            && let Ok(formatted) = serde_json::to_string_pretty(&json) {
                return formatted;
            }
        return html.to_string();
    }

    static RE_SCRIPT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<script[^>]*>[\s\S]*?</script>"#).unwrap());
    static RE_STYLE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<style[^>]*>[\s\S]*?</style>"#).unwrap());
    static RE_NAV: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<nav[^>]*>[\s\S]*?</nav>"#).unwrap());
    static RE_FOOTER: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<footer[^>]*>[\s\S]*?</footer>"#).unwrap());
    static RE_MAIN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<main[^>]*>([\s\S]*?)</main>"#).unwrap());
    static RE_ARTICLE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<article[^>]*>([\s\S]*?)</article>"#).unwrap());
    static RE_H: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<h([1-6])[^>]*>([\s\S]*?)</h[1-6]>"#).unwrap());
    static RE_P: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<p[^>]*>([\s\S]*?)</p>"#).unwrap());
    static RE_BR: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<br\s*/?>"#).unwrap());
    static RE_A: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<a[^>]*href=["']([^"']*?)["'][^>]*>([\s\S]*?)</a>"#).unwrap());
    static RE_LI: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<li[^>]*>([\s\S]*?)</li>"#).unwrap());
    static RE_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"<[^>]+>"#).unwrap());
    static RE_MULTINLINE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"\n\n\n+"#).unwrap());

    let mut text = RE_SCRIPT.replace_all(html, "").into_owned();
    text = RE_STYLE.replace_all(&text, "").into_owned();
    text = RE_NAV.replace_all(&text, "").into_owned();
    text = RE_FOOTER.replace_all(&text, "").into_owned();

    if let Some(caps) = RE_MAIN.captures(&text) {
        text = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
    } else if let Some(caps) = RE_ARTICLE.captures(&text) {
        text = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
    }

    text = RE_H.replace_all(&text, "\n## $2\n").into_owned();
    text = RE_P.replace_all(&text, "\n$1\n").into_owned();
    text = RE_BR.replace_all(&text, "\n").into_owned();
    text = RE_A.replace_all(&text, "$2 ($1)").into_owned();
    text = RE_LI.replace_all(&text, "- $1\n").into_owned();
    text = RE_TAGS.replace_all(&text, "").into_owned();

    // Replace HTML entities
    text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    text = RE_MULTINLINE.replace_all(&text, "\n\n").into_owned();
    text = text.trim().to_string();

    if text.len() > 50000 {
        text[..50000].to_string()
    } else {
        text
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CachedSitemap {
    pub domain: String,
    pub urls: Vec<String>,
    pub fetched_at: String,
}

pub fn get_sitemap_cache_path(project_root: &std::path::Path, domain: &str) -> std::path::PathBuf {
    let clean_domain = domain.replace(|c: char| !c.is_alphanumeric(), "_");
    crate::infrastructure::utils::paths::project_cache_dir(project_root)
        .join("web-sitemaps")
        .join(format!("{}.json", clean_domain))
}

fn urlencode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

fn parse_sitemap_xml(xml: &str) -> Vec<String> {
    static RE_LOC: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<loc>\s*([^\s<]+)\s*</loc>"#).unwrap());
    RE_LOC.captures_iter(xml)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
        .collect()
}

fn parse_sitemap_txt(txt: &str) -> Vec<String> {
    txt.lines()
        .map(|line| line.trim().to_string())
        .filter(|line| line.starts_with("http://") || line.starts_with("https://"))
        .collect()
}

impl WebScraper {
    pub async fn fetch_sitemap_for_domain(&self, domain_url: &str) -> Vec<String> {
        // 1. Fetch robots.txt to find sitemap declaration
        let robots_url = format!("{}/robots.txt", domain_url);
        if let Ok(resp) = self.client.get(&robots_url).send().await
            && resp.status().is_success()
                && let Ok(txt) = resp.text().await {
                    static RE_SITEMAP: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)^sitemap:\s*([^\s]+)"#).unwrap());
                    for line in txt.lines() {
                        if let Some(caps) = RE_SITEMAP.captures(line.trim()) {
                            let sitemap_url = caps.get(1).unwrap().as_str();
                            if let Some(urls) = self.download_sitemap(sitemap_url).await
                                && !urls.is_empty() {
                                    return urls;
                                }
                        }
                    }
                }
        // 2. Fallbacks
        for path in &["/sitemap.xml", "/sitemap.txt", "/sitemap_index.xml"] {
            let sitemap_url = format!("{}{}", domain_url, path);
            if let Some(urls) = self.download_sitemap(&sitemap_url).await
                && !urls.is_empty() {
                    return urls;
                }
        }
        Vec::new()
    }

    async fn download_sitemap(&self, url: &str) -> Option<Vec<String>> {
        if let Ok(resp) = self.client.get(url).send().await
            && resp.status().is_success()
                && let Ok(content) = resp.text().await {
                    let urls = if url.ends_with(".txt") {
                        parse_sitemap_txt(&content)
                    } else {
                        parse_sitemap_xml(&content)
                    };
                    if !urls.is_empty() {
                        return Some(urls);
                    }
                }
        None
    }

    pub async fn search_web(&self, query: &str, project_root: &std::path::Path) -> Result<Vec<SearchResult>, UnevenError> {
        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencode(query));
        let response = self.client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36")
            .send()
            .await
            .map_err(|e| UnevenError::KnowledgeError(format!("Search request failed: {e}")))?;
            
        let html = response.text()
            .await
            .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read search response body: {e}")))?;
            
        let mut results = Vec::new();
        static RE_LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<a[^>]+class="result__a"[^>]+href="([^"]+)"[^>]*>([\s\S]*?)</a>"#).unwrap());
        static RE_SNIPPET: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)<a[^>]+class="result__snippet"[^>]*>([\s\S]*?)</a>"#).unwrap());
        static RE_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"<[^>]+>"#).unwrap());

        let links: Vec<(String, String)> = RE_LINK.captures_iter(&html)
            .map(|caps| (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string()))
            .collect();
            
        let snippets: Vec<String> = RE_SNIPPET.captures_iter(&html)
            .map(|caps| caps.get(1).unwrap().as_str().to_string())
            .collect();
            
        for (idx, (href, title_html)) in links.iter().take(5).enumerate() {
            let mut title = RE_TAGS.replace_all(title_html, "").into_owned();
            title = title.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").trim().to_string();
            
            let raw_snippet = snippets.get(idx).cloned().unwrap_or_default();
            let mut snippet = RE_TAGS.replace_all(&raw_snippet, "").into_owned();
            snippet = snippet.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").trim().to_string();
            
            let mut resolved_url = href.clone();
            if resolved_url.contains("uddg=")
                && let Ok(parsed_url) = reqwest::Url::parse(&format!("https://ddg.gg{}", resolved_url))
                    && let Some((_, val)) = parsed_url.query_pairs().find(|(key, _)| key == "uddg") {
                        resolved_url = val.into_owned();
                    }
            
            results.push(SearchResult {
                title,
                url: resolved_url,
                snippet,
            });
        }
        
        // Trigger background sitemap caching for top search results
        for res in &results {
            if let Ok(parsed_url) = reqwest::Url::parse(&res.url)
                && let Some(host) = parsed_url.host_str() {
                    let scheme = parsed_url.scheme();
                    let domain = format!("{}://{}", scheme, host);
                    let self_clone = self.clone();
                    let project_root_clone = project_root.to_path_buf();
                    let host_str = host.to_string();
                    tokio::spawn(async move {
                        let cache_path = get_sitemap_cache_path(&project_root_clone, &host_str);
                        if !cache_path.exists() {
                            let urls = self_clone.fetch_sitemap_for_domain(&domain).await;
                            if !urls.is_empty() {
                                let cached = CachedSitemap {
                                    domain: host_str,
                                    urls,
                                    fetched_at: chrono::Utc::now().to_rfc3339(),
                                };
                                if let Some(parent) = cache_path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                if let Ok(json) = serde_json::to_string_pretty(&cached) {
                                    let _ = std::fs::write(&cache_path, json);
                                }
                            }
                        }
                    });
                }
        }
        
        Ok(results)
    }

    pub async fn search_wikipedia(&self, query: &str) -> Result<Vec<SearchResult>, UnevenError> {
        let url = format!("https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&utf8=&format=json", urlencode(query));
        let response = self.client.get(&url)
            .header("User-Agent", &self.user_agent)
            .send()
            .await
            .map_err(|e| UnevenError::KnowledgeError(format!("Wikipedia request failed: {e}")))?;
            
        let body = response.text()
            .await
            .map_err(|e| UnevenError::KnowledgeError(format!("Failed to read Wikipedia response body: {e}")))?;
            
        let mut results = Vec::new();
        static RE_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"<[^>]+>"#).unwrap());
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(items) = json["query"]["search"].as_array() {
                for item in items.iter().take(3) {
                    let title = item["title"].as_str().unwrap_or("").to_string();
                    let raw_snippet = item["snippet"].as_str().unwrap_or("").to_string();
                    let snippet = RE_TAGS.replace_all(&raw_snippet, "").into_owned();
                    let pageid = item["pageid"].as_u64().unwrap_or(0);
                    let url = format!("https://en.wikipedia.org/?curid={}", pageid);
                    results.push(SearchResult {
                        title,
                        url,
                        snippet,
                    });
                }
            }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a32() {
        assert_eq!(fnv1a32("hello"), 1335831723);
        assert_eq!(fnv1a32("https://google.com/path"), 2236989232);
    }

    #[test]
    fn test_url_to_id() {
        let id = url_to_id("https://google.com/search?q=test");
        assert!(id.starts_with("web_google_com__search_"));
    }

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>My Awesome Page</title></head><body></body></html>";
        assert_eq!(extract_title(html), "My Awesome Page");

        let html_no_title = "<html><body><h1>Page H1</h1></body></html>";
        assert_eq!(extract_title(html_no_title), "Page H1");
    }

    #[test]
    fn test_extract_content() {
        let html = r#"
            <html>
                <head><style>body { color: red; }</style></head>
                <body>
                    <nav>Link 1 | Link 2</nav>
                    <main>
                        <h1>Welcome</h1>
                        <p>This is a <b>paragraph</b> with <a href="/test">link</a>.</p>
                        <ul>
                            <li>Item 1</li>
                        </ul>
                    </main>
                    <footer>Copyright 2026</footer>
                </body>
            </html>
        "#;
        let content = extract_content(html, "text/html");
        assert!(content.contains("## Welcome"));
        assert!(content.contains("This is a paragraph with link (/test)."));
        assert!(content.contains("- Item 1"));
        assert!(!content.contains("Link 1 | Link 2"));
        assert!(!content.contains("Copyright 2026"));
    }
}
