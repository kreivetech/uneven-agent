use crate::UnevenError;
use std::process::Command;
use std::fs;
use std::path::Path;

pub fn package_to_exe(html_content: &str, title: &str, output_path: &Path) -> Result<(), UnevenError> {
    let tmp_dir = std::env::temp_dir().join(format!("uneven-pkg-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp_dir).map_err(UnevenError::IoError)?;

    let viewer_path = tmp_dir.join("viewer.js");
    let viewer_script = build_viewer_script(html_content, title);
    fs::write(&viewer_path, viewer_script).map_err(UnevenError::IoError)?;

    let status = Command::new("npx")
        .args([
            "pkg",
            viewer_path.to_str().unwrap(),
            "--targets", "node18-linux-x64", // Defaulting to linux-x64 for now
            "--output", output_path.to_str().unwrap(),
            "--compress", "GZip",
        ])
        .status()
        .map_err(|e| UnevenError::KnowledgeError(format!("Failed to run pkg: {}", e)))?;

    if !status.success() {
        return Err(UnevenError::KnowledgeError(format!("pkg failed with status: {}", status)));
    }

    // Cleanup
    let _ = fs::remove_dir_all(tmp_dir);

    Ok(())
}

fn build_viewer_script(html: &str, title: &str) -> String {
    let escaped_html = html
        .replace("\\", "\\\\")
        .replace("`", "\\`")
        .replace("${", "\\${");

    format!(
        r#"'use strict';
const http = require('http');
const {{ exec }} = require('child_process');
const os = require('os');

const TITLE = {:?};
const HTML = `{}`;
const TIMEOUT_MS = 30 * 60 * 1000;

let inactivityTimer;
function resetTimer(server) {{
  clearTimeout(inactivityTimer);
  inactivityTimer = setTimeout(() => {{
    server.close(() => process.exit(0));
  }}, TIMEOUT_MS);
}}

const server = http.createServer((req, res) => {{
  resetTimer(server);
  if (req.url === '/close') {{
    res.writeHead(200, {{ 'Content-Type': 'text/plain' }});
    res.end('Dashboard closed.');
    setTimeout(() => {{ server.close(() => process.exit(0)); }}, 300);
    return;
  }}
  res.writeHead(200, {{ 'Content-Type': 'text/html; charset=utf-8' }});
  res.end(HTML);
}});

server.listen(0, '127.0.0.1', () => {{
  const {{ port }} = server.address();
  const url = `http://127.0.0.1:${{port}}`;
  console.log('  Uneven Dashboard Report');
  console.log('  Title    : ' + TITLE);
  console.log('  URL      : ' + url);

  const platform = os.platform();
  const cmd = platform === 'win32' ? `start "" "${{url}}"` : platform === 'darwin' ? `open "${{url}}"` : `xdg-open "${{url}}"`;
  exec(cmd);
  resetTimer(server);
}});
"#,
        title, escaped_html
    )
}
