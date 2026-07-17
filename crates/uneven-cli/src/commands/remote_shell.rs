use anyhow::Result;
use console::style;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};

use uneven_core::infrastructure::config::loader;
use uneven_core::domain::config::RemoteShellConfig;
use uneven_core::UnevenConfig;
use crate::commands::license;

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteShellResult {
    pub action: String,
    pub message: String,
    pub output: String,
    pub status: String,
}

struct ClassifiedIntent {
    action: String,
    args: Vec<String>,
    is_async: bool,
}

pub async fn run() -> Result<()> {
    license::require_team()?;
    let root = loader::find_project_root(None);
    let config = loader::load_config(&root)?;

    let shell_config = match config.remote_shell {
        Some(c) => c,
        None => {
            return Err(anyhow::anyhow!(
                "[UE-02000001] remoteShell is not configured. Please use '/' in the interactive shell to configure settings."
            ));
        }
    };

    run_server(shell_config).await
}

fn extract_file_path(input: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:^|\s)((?:\.{1,2}/)?(?:[\w.-]+/)*[\w.-]+\.[\w]{1,6})(?:\s|$|[?!,.])").ok()?;
    re.captures(input).map(|c| c[1].to_string())
}

fn looks_like_question(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.ends_with('?') {
        return true;
    }
    let starters = [
        "what ", "how ", "why ", "when ", "where ", "who ", "which ", "is ", "are ", "does ",
        "do ", "can ", "could ", "should ", "would ", "will ",
        "o que ", "como ", "por que ", "porque ", "quando ", "onde ", "quem ", "qual ", "quais ",
        "é ", "são ", "será ", "existe ", "tem ", "há ",
        "qué ", "cómo ", "por qué ", "cuándo ", "dónde ", "quién ", "cuál ",
    ];
    let low = trimmed.to_lowercase();
    starters.iter().any(|s| low.starts_with(s))
}

fn classify_intent(raw: &str) -> ClassifiedIntent {
    let input = raw.trim();
    let low = input.to_lowercase();
    let first = low.split_whitespace().next().unwrap_or("");

    if ["exit", "quit", "bye"].iter().any(|w| low.contains(w)) {
        return ClassifiedIntent { action: "exit".to_string(), args: vec![], is_async: false };
    }
    if low.contains("help") {
        return ClassifiedIntent { action: "help".to_string(), args: vec![], is_async: false };
    }
    if low.contains("log") {
        return ClassifiedIntent { action: "log".to_string(), args: vec![], is_async: false };
    }
    if low.contains("undo") {
        return ClassifiedIntent { action: "undo".to_string(), args: vec![], is_async: false };
    }

    if first == "index" {
        return ClassifiedIntent { action: "index".to_string(), args: vec![], is_async: false };
    }
    if first == "scan" {
        return ClassifiedIntent { action: "scan".to_string(), args: vec![], is_async: false };
    }
    if first == "watch" {
        return ClassifiedIntent { action: "watch".to_string(), args: vec![], is_async: true };
    }

    if first == "review" {
        let mut args = vec![];
        let hash_re = regex::Regex::new(r"\b([0-9a-f]{7,40})\b").unwrap();
        let from_re = regex::Regex::new(r"\b(main|master|HEAD~\d+|HEAD\^\d*)\b").unwrap();
        if let Some(cap) = hash_re.captures(input) {
            args.push("--commit".to_string());
            args.push(cap[1].to_string());
        } else if let Some(cap) = from_re.captures(input) {
            args.push("--from".to_string());
            args.push(cap[1].to_string());
        }
        if low.contains("staged") {
            args.push("--staged".to_string());
        }
        return ClassifiedIntent { action: "review".to_string(), args, is_async: false };
    }

    if first == "docs" {
        let mut args = vec![];
        if let Some(file) = extract_file_path(input) {
            args.push(file);
        }
        return ClassifiedIntent { action: "docs".to_string(), args, is_async: false };
    }

    if first == "explain" {
        let mut args = vec![];
        if let Some(file) = extract_file_path(input) {
            args.push(file);
        }
        return ClassifiedIntent { action: "explain".to_string(), args, is_async: false };
    }

    if let Some(file) = extract_file_path(input) {
        if input == file {
            return ClassifiedIntent { action: "explain".to_string(), args: vec![file], is_async: false };
        }
    }

    if looks_like_question(input) {
        return ClassifiedIntent { action: "ask".to_string(), args: vec![input.to_string()], is_async: false };
    }

    ClassifiedIntent { action: "askf".to_string(), args: vec![input.to_string()], is_async: true }
}

async fn execute_intent(intent: ClassifiedIntent) -> RemoteShellResult {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            return RemoteShellResult {
                action: intent.action,
                message: "Failed to locate executable".to_string(),
                output: e.to_string(),
                status: "error".to_string(),
            };
        }
    };

    let mut args = vec![];
    args.push(intent.action.clone());
    args.extend(intent.args);

    if intent.is_async {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&args);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        
        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        
        match cmd.spawn() {
            Ok(_) => RemoteShellResult {
                action: intent.action.clone(),
                message: format!("Command '{}' started in the background.", intent.action),
                output: String::new(),
                status: "async".to_string(),
            },
            Err(e) => RemoteShellResult {
                action: intent.action,
                message: "Failed to spawn command in background".to_string(),
                output: e.to_string(),
                status: "error".to_string(),
            }
        }
    } else {
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(&args);
        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else if stdout.is_empty() {
                    stderr
                } else {
                    format!("{}\n{}", stdout, stderr)
                };
                
                let success = output.status.success();
                RemoteShellResult {
                    action: intent.action.clone(),
                    message: format!("Command '{}' completed.", intent.action),
                    output: combined,
                    status: if success { "ok".to_string() } else { "error".to_string() },
                }
            }
            Err(e) => RemoteShellResult {
                action: intent.action,
                message: "Failed to execute command".to_string(),
                output: e.to_string(),
                status: "error".to_string(),
            }
        }
    }
}

enum MaybeTlsStream {
    Plain(tokio::net::TcpStream),
    Tls(Box<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>),
}

impl tokio::io::AsyncRead for MaybeTlsStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Tls(s) => std::pin::Pin::new(s.as_mut()).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for MaybeTlsStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Tls(s) => std::pin::Pin::new(s.as_mut()).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Tls(s) => std::pin::Pin::new(s.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::Tls(s) => std::pin::Pin::new(s.as_mut()).poll_shutdown(cx),
        }
    }
}

fn load_certs(path: &str) -> Result<Vec<tokio_rustls::rustls::pki_types::CertificateDer<'static>>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::io::Result<Vec<_>>>()?;
    Ok(certs)
}

fn load_key(path: &str) -> Result<tokio_rustls::rustls::pki_types::PrivateKeyDer<'static>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let key = rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| anyhow::anyhow!("No private key found in {}", path))?;
    Ok(key)
}

struct IpLockInfo {
    failures: usize,
    last_failure: Instant,
}

fn is_ip_locked(tracker: &Arc<Mutex<HashMap<String, IpLockInfo>>>, ip: &str) -> bool {
    let map = tracker.lock().unwrap();
    if let Some(info) = map.get(ip) {
        if info.failures >= 5 && info.last_failure.elapsed() < Duration::from_secs(300) {
            return true;
        }
    }
    false
}

fn record_auth_failure(tracker: &Arc<Mutex<HashMap<String, IpLockInfo>>>, ip: String) {
    let mut map = tracker.lock().unwrap();
    let entry = map.entry(ip).or_insert(IpLockInfo {
        failures: 0,
        last_failure: Instant::now(),
    });
    if entry.last_failure.elapsed() >= Duration::from_secs(300) {
        entry.failures = 1;
    } else {
        entry.failures += 1;
    }
    entry.last_failure = Instant::now();
}

fn reset_auth_failures(tracker: &Arc<Mutex<HashMap<String, IpLockInfo>>>, ip: &str) {
    let mut map = tracker.lock().unwrap();
    map.remove(ip);
}

fn merge_json(a: &mut serde_json::Value, b: &serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(ref mut a_map), serde_json::Value::Object(ref b_map)) => {
            for (k, v) in b_map {
                if let Some(a_val) = a_map.get_mut(k) {
                    merge_json(a_val, v);
                } else {
                    a_map.insert(k.clone(), v.clone());
                }
            }
        }
        (a_val, b_val) => {
            *a_val = b_val.clone();
        }
    }
}

pub async fn run_server(config: RemoteShellConfig) -> Result<()> {
    let port = config.port;
    let secret = config.secret.clone();
    let allowed_origins = config.cors.as_ref().and_then(|c| c.allowed_origins.clone());

    let acceptor = if let Some(ref https_cfg) = config.https {
        let certs = load_certs(&https_cfg.cert)?;
        let key = load_key(&https_cfg.key)?;
        let provider = tokio_rustls::rustls::crypto::ring::default_provider();
        let server_config = tokio_rustls::rustls::ServerConfig::builder_with_provider(Arc::new(provider))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Some(tokio_rustls::TlsAcceptor::from(Arc::new(server_config)))
    } else {
        None
    };

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;

    let proto = if acceptor.is_some() { "https" } else { "http" };

    println!("\n  ┌─────────────────────────────────────────────────┐");
    println!("  │  ◈  UNEVEN AGENT REMOTE SHELL                     │");
    println!("  │  Listening for messages from external tools     │");
    println!("  └─────────────────────────────────────────────────┘\n");
    println!("  {} Server running on {}://{}", style("✓").green().bold(), proto, addr);
    println!("  {} Endpoints:", style("◈").dim());
    println!("    {} POST /message    {}", style("POST").cyan(), style("{ \"text\": \"index my project\" }").dim());
    println!("    {} GET  /health     {}", style("GET ").cyan(), style("→ { \"status\": \"ok\", \"version\": \"...\" }").dim());
    
    if secret.is_some() {
        println!("  {} Auth required: Authorization: Bearer <secret>", style("🔒").yellow());
    }
    println!("  Press Ctrl+C to stop.\n");

    let rate_limit_map = Arc::new(Mutex::new(HashMap::<String, Vec<Instant>>::new()));
    let limit_rpm = config.rate_limit.as_ref().and_then(|r| r.requests_per_minute).unwrap_or(60);
    let brute_force_tracker = Arc::new(Mutex::new(HashMap::<String, IpLockInfo>::new()));
    let chat_sessions = Arc::new(Mutex::new(HashMap::<String, Vec<(String, String)>>::new()));

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(val) => val,
            Err(_) => continue,
        };

        let secret_clone = secret.clone();
        let origins_clone = allowed_origins.clone();
        let rate_limit_map_clone = rate_limit_map.clone();
        let brute_force_tracker_clone = brute_force_tracker.clone();
        let tls_acceptor_clone = acceptor.clone();
        let chat_sessions_clone = chat_sessions.clone();

        let ip = match peer_addr {
            std::net::SocketAddr::V4(a) => a.ip().to_string(),
            std::net::SocketAddr::V6(a) => a.ip().to_string(),
        };

        tokio::spawn(async move {
            let mut stream = if let Some(ref tls_acceptor) = tls_acceptor_clone {
                match tls_acceptor.accept(stream).await {
                    Ok(tls_stream) => MaybeTlsStream::Tls(Box::new(tls_stream)),
                    Err(e) => {
                        eprintln!("TLS handshake failed: {}", e);
                        return;
                    }
                }
            } else {
                MaybeTlsStream::Plain(stream)
            };

            if is_ip_locked(&brute_force_tracker_clone, &ip) {
                let body = "{\"error\":\"Locked\",\"code\":\"UE-02000004\",\"message\":\"Too many auth failures. Locked for 5 min.\"}";
                let res = format!(
                    "HTTP/1.1 423 Locked\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(res.as_bytes()).await;
                return;
            }

            let mut buf = [0; 4096];
            let mut request_bytes = Vec::new();
            
            // Read until headers end
            loop {
                let n = match stream.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };
                request_bytes.extend_from_slice(&buf[..n]);
                if request_bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                if request_bytes.len() > 16384 {
                    return;
                }
            }

            let request_str = String::from_utf8_lossy(&request_bytes);
            let mut lines = request_str.lines();
            
            let request_line = match lines.next() {
                Some(line) => line,
                None => return,
            };

            let parts: Vec<&str> = request_line.split_whitespace().collect();
            if parts.len() < 3 {
                return;
            }
            let method = parts[0];
            let path = parts[1];

            // Parse headers
            let mut headers = HashMap::new();
            for line in lines {
                if line.is_empty() {
                    break;
                }
                let header_parts: Vec<&str> = line.splitn(2, ':').collect();
                if header_parts.len() == 2 {
                    headers.insert(
                        header_parts[0].trim().to_lowercase(),
                        header_parts[1].trim().to_string(),
                    );
                }
            }

            // CORS verification
            if let Some(ref origins) = origins_clone {
                if let Some(origin) = headers.get("origin") {
                    if !origins.contains(origin) {
                        let res = "HTTP/1.1 403 Forbidden\r\nContent-Length: 22\r\n\r\n{\"error\":\"Forbidden\"}";
                        let _ = stream.write_all(res.as_bytes()).await;
                        return;
                    }
                }
            }

            // Authentication
            if let Some(ref sec) = secret_clone {
                let auth_header = headers.get("authorization").cloned().unwrap_or_default();
                let provided = auth_header.replace("Bearer ", "");
                if provided != *sec {
                    record_auth_failure(&brute_force_tracker_clone, ip.clone());
                    let res = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 25\r\n\r\n{\"error\":\"Unauthorized\"}";
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                } else {
                    reset_auth_failures(&brute_force_tracker_clone, &ip);
                }
            }

            // GET /health
            if method == "GET" && path == "/health" {
                let body = "{\"status\":\"ok\",\"version\":\"2.0.0\"}";
                let res = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(res.as_bytes()).await;
                return;
            }

            // GET /config
            if method == "GET" && path == "/config" {
                let root = loader::find_project_root(None);
                match loader::load_config(&root) {
                    Ok(config) => {
                        let body = serde_json::to_string(&config).unwrap_or_default();
                        let res = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(res.as_bytes()).await;
                    }
                    Err(e) => {
                        let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                        let body = format!("{{\"error\":\"{}\"}}", sanitized);
                        let res = format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(res.as_bytes()).await;
                    }
                }
                return;
            }

            // POST /config
            if method == "POST" && path == "/config" {
                let content_length = headers
                    .get("content-length")
                    .and_then(|l| l.parse::<usize>().ok())
                    .unwrap_or(0);

                if content_length > 65536 {
                    let res = "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                let header_end_idx = match request_str.find("\r\n\r\n") {
                    Some(idx) => idx + 4,
                    None => return,
                };
                let mut body_bytes = request_bytes[header_end_idx..].to_vec();
                while body_bytes.len() < content_length {
                    let n = match stream.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => break,
                    };
                    body_bytes.extend_from_slice(&buf[..n]);
                }

                let body_str = String::from_utf8_lossy(&body_bytes[..content_length.min(body_bytes.len())]);
                let patch: serde_json::Value = match serde_json::from_str(&body_str) {
                    Ok(val) => val,
                    Err(_) => {
                        let res = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(res.as_bytes()).await;
                        return;
                    }
                };

                let root = loader::find_project_root(None);
                let config_res = loader::load_config(&root);
                match config_res {
                    Ok(config) => {
                        let mut config_val = serde_json::to_value(&config).unwrap_or_default();
                        merge_json(&mut config_val, &patch);
                        
                        match serde_json::from_value::<UnevenConfig>(config_val) {
                            Ok(updated_config) => {
                                let config_path = uneven_core::infrastructure::utils::paths::project_cache_dir(&root).join("config.json");
                                if let Some(parent) = config_path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                if let Ok(config_json) = serde_json::to_string_pretty(&updated_config) {
                                    let _ = std::fs::write(&config_path, config_json);
                                }
                                
                                let body = serde_json::to_string(&updated_config).unwrap_or_default();
                                let res = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(res.as_bytes()).await;
                            }
                            Err(e) => {
                                let body = format!("{{\"error\":\"Invalid config structure: {}\"}}", e);
                                let res = format!(
                                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(res.as_bytes()).await;
                            }
                        }
                    }
                    Err(e) => {
                        let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                        let body = format!("{{\"error\":\"{}\"}}", sanitized);
                        let res = format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(res.as_bytes()).await;
                    }
                }
                return;
            }

            // POST /chat
            if method == "POST" && path == "/chat" {
                // Get Content-Length
                let content_length = headers
                    .get("content-length")
                    .and_then(|l| l.parse::<usize>().ok())
                    .unwrap_or(0);

                if content_length > 65536 {
                    let res = "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                // Read request body
                let header_end_idx = match request_str.find("\r\n\r\n") {
                    Some(idx) => idx + 4,
                    None => return,
                };
                let mut body_bytes = request_bytes[header_end_idx..].to_vec();
                while body_bytes.len() < content_length {
                    let n = match stream.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => break,
                    };
                    body_bytes.extend_from_slice(&buf[..n]);
                }

                let body_str = String::from_utf8_lossy(&body_bytes[..content_length.min(body_bytes.len())]);
                let parsed: serde_json::Value = match serde_json::from_str(&body_str) {
                    Ok(val) => val,
                    Err(_) => {
                        let res = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(res.as_bytes()).await;
                        return;
                    }
                };

                let text = parsed.get("text").and_then(|t| t.as_str()).unwrap_or("").trim();
                if text.is_empty() {
                    let res = "HTTP/1.1 400 Bad Request\r\nContent-Length: 29\r\n\r\n{\"error\":\"text is required\"}";
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                let mut session_id = headers.get("x-session-id").cloned()
                    .or_else(|| parsed.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
                    .unwrap_or_default();
                if session_id.is_empty() {
                    let millis = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    session_id = format!("sess_{}", millis);
                }

                // Rate limiting based on session_id
                let is_limited = {
                    let mut map = rate_limit_map_clone.lock().unwrap();
                    let now = Instant::now();
                    let timestamps = map.entry(session_id.clone()).or_default();
                    timestamps.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
                    if timestamps.len() >= limit_rpm as usize {
                        true
                    } else {
                        timestamps.push(now);
                        false
                    }
                };

                if is_limited {
                    let body = "{\"error\":\"Too Many Requests\"}";
                    let res = format!(
                        "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                let root = loader::find_project_root(None);
                let config = match loader::load_config(&root) {
                    Ok(c) => c,
                    Err(e) => {
                        let body = format!("{{\"error\":\"Failed to load config: {}\"}}", e);
                        let res = format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(res.as_bytes()).await;
                        return;
                    }
                };

                match super::shell_engine::handle_chat_message(&session_id, text, &config, chat_sessions_clone).await {
                    Ok(reply) => {
                        let response_body = serde_json::json!({
                            "message": reply,
                            "id": session_id
                        }).to_string();

                        let res = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(res.as_bytes()).await;
                    }
                    Err(e) => {
                        let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                        let body = format!("{{\"error\":\"{}\"}}", sanitized);
                        let res = format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(res.as_bytes()).await;
                    }
                }
                return;
            }

            // POST /message
            if method == "POST" && path == "/message" {
                let session_id = headers.get("x-session-id").cloned().unwrap_or_else(|| "default".to_string());

                // Rate limiting
                let is_limited = {
                    let mut map = rate_limit_map_clone.lock().unwrap();
                    let now = Instant::now();
                    let timestamps = map.entry(session_id.clone()).or_default();
                    timestamps.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
                    if timestamps.len() >= limit_rpm as usize {
                        true
                    } else {
                        timestamps.push(now);
                        false
                    }
                };

                if is_limited {
                    let body = "{\"error\":\"Too Many Requests\"}";
                    let res = format!(
                        "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                // Get Content-Length
                let content_length = headers
                    .get("content-length")
                    .and_then(|l| l.parse::<usize>().ok())
                    .unwrap_or(0);

                if content_length > 65536 {
                    let res = "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                // Read request body
                let header_end_idx = match request_str.find("\r\n\r\n") {
                    Some(idx) => idx + 4,
                    None => return,
                };
                let mut body_bytes = request_bytes[header_end_idx..].to_vec();
                while body_bytes.len() < content_length {
                    let n = match stream.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => break,
                    };
                    body_bytes.extend_from_slice(&buf[..n]);
                }

                let body_str = String::from_utf8_lossy(&body_bytes[..content_length.min(body_bytes.len())]);
                let parsed: serde_json::Value = match serde_json::from_str(&body_str) {
                    Ok(val) => val,
                    Err(_) => {
                        let res = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(res.as_bytes()).await;
                        return;
                    }
                };

                let text = parsed.get("text").and_then(|t| t.as_str()).unwrap_or("").trim();
                if text.is_empty() {
                    let res = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(res.as_bytes()).await;
                    return;
                }

                if text.starts_with('/') {
                    let root = loader::find_project_root(None);
                    let config_res = loader::load_config(&root);
                    match config_res {
                        Ok(config) => {
                            match super::shell_engine::handle_slash_command(text, &config, &root) {
                                Ok(reply) => {
                                    let result = RemoteShellResult {
                                        action: "config".to_string(),
                                        message: reply.clone(),
                                        output: reply,
                                        status: "ok".to_string(),
                                    };
                                    let response_body = serde_json::to_string(&result).unwrap_or_default();
                                    let res = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        response_body.len(),
                                        response_body
                                    );
                                    let _ = stream.write_all(res.as_bytes()).await;
                                }
                                Err(e) => {
                                    let result = RemoteShellResult {
                                        action: "config".to_string(),
                                        message: format!("Error: {}", e),
                                        output: e.to_string(),
                                        status: "error".to_string(),
                                    };
                                    let response_body = serde_json::to_string(&result).unwrap_or_default();
                                    let res = format!(
                                        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        response_body.len(),
                                        response_body
                                    );
                                    let _ = stream.write_all(res.as_bytes()).await;
                                }
                            }
                        }
                        Err(e) => {
                            let sanitized = crate::errors::sanitize_error_msg(&e.to_string());
                            let body = format!("{{\"error\":\"{}\"}}", sanitized);
                            let res = format!(
                                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(res.as_bytes()).await;
                        }
                    }
                    return;
                }

                let intent = classify_intent(text);
                let result = execute_intent(intent).await;
                
                let response_body = match serde_json::to_string(&result) {
                    Ok(body) => body,
                    Err(_) => "{\"error\":\"Internal Server Error\"}".to_string(),
                };

                let res = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(res.as_bytes()).await;
                return;
            }

            // GET /
            if method == "GET" && path == "/" {
                let help_json = serde_json::json!({
                    "service": "Uneven Remote Shell",
                    "version": "2.0.0",
                    "endpoints": {
                        "/health": {
                            "method": "GET",
                            "description": "Check server health status."
                        },
                        "/config": {
                            "method": "GET / POST",
                            "description": "Retrieve or dynamically patch/update the global JSON configuration.",
                            "payload_post": "Partial UnevenConfig JSON schema"
                        },
                        "/chat": {
                            "method": "POST",
                            "description": "Conversational unified shell chat with context-optimized history. Supports interactive slash commands.",
                            "payload": {
                                "text": "user message or slash command (required)",
                                "id": "session_id (optional)"
                            }
                        },
                        "/message": {
                            "method": "POST",
                            "description": "Execute stateless CLI commands or interactive slash commands.",
                            "payload": {
                                "text": "command text or slash command (required)"
                            }
                        }
                    }
                }).to_string();

                let res = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    help_json.len(),
                    help_json
                );
                let _ = stream.write_all(res.as_bytes()).await;
                return;
            }

            let res = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(res.as_bytes()).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_lock_logic() {
        let tracker = Arc::new(Mutex::new(HashMap::new()));
        let ip = "127.0.0.1".to_string();

        assert!(!is_ip_locked(&tracker, &ip));

        // Record 4 failures
        for _ in 0..4 {
            record_auth_failure(&tracker, ip.clone());
        }
        assert!(!is_ip_locked(&tracker, &ip));

        // Record 5th failure -> should lock
        record_auth_failure(&tracker, ip.clone());
        assert!(is_ip_locked(&tracker, &ip));

        // Reset failures -> should unlock
        reset_auth_failures(&tracker, &ip);
        assert!(!is_ip_locked(&tracker, &ip));
    }

    #[test]
    fn test_merge_json() {
        let mut original = serde_json::json!({
            "brain": {
                "provider": "openai",
                "model": "gpt-4o",
                "maxTokens": 2048
            },
            "persona": "Default Persona"
        });

        let patch = serde_json::json!({
            "brain": {
                "model": "gpt-5.5",
                "maxTokens": 4096
            },
            "persona": "New Persona"
        });

        merge_json(&mut original, &patch);

        assert_eq!(original["brain"]["provider"].as_str().unwrap(), "openai");
        assert_eq!(original["brain"]["model"].as_str().unwrap(), "gpt-5.5");
        assert_eq!(original["brain"]["maxTokens"].as_i64().unwrap(), 4096);
        assert_eq!(original["persona"].as_str().unwrap(), "New Persona");
    }
}

