use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;
use crate::infrastructure::agent::strategies::detector;

pub fn suggest(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    let ext = std::path::Path::new(&error.file)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if !["js", "jsx", "ts", "tsx"].contains(&ext.as_str()) {
        return None;
    }

    let proj_root = detector::find_project_root(&error.file);
    let is_next_project = detector::has_npm_dependency(&proj_root, "next");

    let has_next_signature = error.message.contains("Next.js")
        || error.message.contains("next/router")
        || error.message.contains("next/navigation")
        || (error.message.contains("useState") && error.message.contains("Server Component"))
        || (error.message.contains("useState") && error.message.contains("Client Component"))
        || (error.message.contains("params") && error.message.contains("Promise"));

    if !is_next_project && !has_next_signature {
        return None;
    }

    // 1. React Hook used in Server Component (requires "use client")
    if (error.message.contains("useState") || error.message.contains("useEffect") || error.message.contains("useContext"))
        && (error.message.contains("Server Component")
            || error.message.contains("only works in a Client Component")
            || error.message.contains("client-only"))
    {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: 1,
            before: "".to_string(),
            after: "\"use client\";\n\n".to_string(),


            confidence: 0.95,
            append_eof: false,
            top_of_file: true,
            explanation: "Next.js: React hooks can only be used in Client Components.".to_string(),
            suggested_fix: "Injected \"use client\" directive at the top of the file".to_string(),
        });
    }

    // 2. Next.js 15: Page params or searchParams must be awaited (TS/JS error)
    if error.message.contains("params")
        && (error.message.contains("should be awaited")
            || error.message.contains("property does not exist on type Promise")
            || error.message.contains("Promise"))
    {
        return Some(FixSuggestion {
            file: error.file.clone(),
            line: error.line,
            before: "".to_string(),
            after: "// Example:\nasync function Page({ params }: { params: Promise<any> }) {\n  const resolvedParams = await params;\n}".to_string(),
            confidence: 0.4,
            append_eof: false,
            top_of_file: false,
            explanation: "Next.js 15: Page route parameters (params or searchParams) are now Promises and must be awaited.".to_string(),
            suggested_fix: "Make the page/layout function async and await params before destructuring or reading properties".to_string(),
        });
    }

    // 3. Image missing width/height
    if error.message.contains("Image") && error.message.contains("missing required") && (error.message.contains("width") || error.message.contains("height")) {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = error.line.saturating_sub(1);
        if let Some(&original_line) = lines.get(line_idx) {
            let target = "<Image";
            let replacement = "<Image width={500} height={500}";
            if original_line.contains(target) {
                return Some(FixSuggestion {
                    file: error.file.clone(),
                    line: error.line,
                    before: original_line.to_string(),
                    after: original_line.replace(target, replacement),


                    confidence: 0.85,
            append_eof: false,
            top_of_file: false,
                    explanation: "Next.js: next/image requires explicit width and height".to_string(),
                    suggested_fix: "Injected default width and height props".to_string(),
                });
            }
        }
    }

    None
}
