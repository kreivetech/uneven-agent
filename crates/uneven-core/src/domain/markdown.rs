use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    Code(String),
    Italic(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarkdownBlock {
    Heading { level: usize, text: String },
    Paragraph(Vec<InlineSpan>),
    BulletItem(Vec<InlineSpan>),
    NumberedItem { number: String, spans: Vec<InlineSpan> },
    CodeBlock {
        lang: String,
        code: String,
        tokenized_lines: Vec<Vec<CodeToken>>,
    },
    EmptyLine,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CodeToken {
    Keyword(String),
    Type(String),
    StringLiteral(String),
    Comment(String),
    Number(String),
    Plain(String),
}

pub fn tokenize_code_line(line: &str, lang: &str) -> Vec<CodeToken> {
    let lang_lower = lang.to_lowercase();
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let indent = &line[..indent_len];

    let mut tokens = Vec::new();
    if !indent.is_empty() {
        tokens.push(CodeToken::Plain(indent.to_string()));
    }

    if trimmed.is_empty() {
        return tokens;
    }

    let comment_prefix = match lang_lower.as_str() {
        "python" | "py" | "bash" | "sh" | "shell" | "zsh" | "toml" | "yaml" | "yml" => Some("#"),
        "sql" => Some("--"),
        _ => Some("//"),
    };

    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;
    let mut current_word = String::new();

    while i < chars.len() {
        // Comment detection
        if let Some(cp) = comment_prefix {
            let cp_chars: Vec<char> = cp.chars().collect();
            if i + cp_chars.len() <= chars.len() && chars[i..i + cp_chars.len()] == cp_chars[..] {
                if !current_word.is_empty() {
                    classify_and_push_word(&mut tokens, &current_word, &lang_lower);
                    current_word.clear();
                }
                let comment_text: String = chars[i..].iter().collect();
                tokens.push(CodeToken::Comment(comment_text));
                return tokens;
            }
        }

        let c = chars[i];

        // String detection
        if c == '"' || c == '\'' || c == '`' {
            if !current_word.is_empty() {
                classify_and_push_word(&mut tokens, &current_word, &lang_lower);
                current_word.clear();
            }
            let quote = c;
            let mut string_literal = String::new();
            string_literal.push(quote);
            i += 1;
            let mut escaped = false;
            while i < chars.len() {
                let ch = chars[i];
                string_literal.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(CodeToken::StringLiteral(string_literal));
            continue;
        }

        // Word vs Delimiter
        if c.is_alphanumeric() || c == '_' {
            current_word.push(c);
            i += 1;
        } else {
            if !current_word.is_empty() {
                classify_and_push_word(&mut tokens, &current_word, &lang_lower);
                current_word.clear();
            }
            tokens.push(CodeToken::Plain(c.to_string()));
            i += 1;
        }
    }

    if !current_word.is_empty() {
        classify_and_push_word(&mut tokens, &current_word, &lang_lower);
    }

    tokens
}

fn classify_and_push_word(tokens: &mut Vec<CodeToken>, word: &str, lang: &str) {
    if is_keyword(word, lang) {
        tokens.push(CodeToken::Keyword(word.to_string()));
    } else if is_boolean_or_number(word) {
        tokens.push(CodeToken::Number(word.to_string()));
    } else if is_type_name(word, lang) {
        tokens.push(CodeToken::Type(word.to_string()));
    } else {
        tokens.push(CodeToken::Plain(word.to_string()));
    }
}

fn is_keyword(word: &str, lang: &str) -> bool {
    match lang {
        "rust" | "rs" => matches!(
            word,
            "pub" | "fn" | "let" | "mut" | "struct" | "enum" | "impl" | "trait" | "async"
                | "await" | "use" | "mod" | "match" | "if" | "else" | "return" | "for"
                | "in" | "while" | "loop" | "break" | "continue" | "const" | "static"
                | "self" | "super" | "crate" | "ref" | "move" | "where" | "type" | "unsafe"
        ),
        "typescript" | "ts" | "javascript" | "js" | "tsx" | "jsx" => matches!(
            word,
            "const" | "let" | "var" | "function" | "return" | "if" | "else" | "import"
                | "export" | "from" | "default" | "class" | "extends" | "implements"
                | "interface" | "type" | "async" | "await" | "new" | "this" | "try"
                | "catch" | "finally" | "throw" | "typeof" | "instanceof" | "switch" | "case"
        ),
        "python" | "py" => matches!(
            word,
            "def" | "class" | "import" | "from" | "return" | "if" | "elif" | "else"
                | "for" | "in" | "while" | "try" | "except" | "finally" | "raise"
                | "with" | "as" | "lambda" | "pass" | "yield" | "self" | "async" | "await"
        ),
        "bash" | "sh" | "shell" | "zsh" => matches!(
            word,
            "echo" | "cd" | "mkdir" | "rm" | "cp" | "mv" | "cat" | "grep" | "find"
                | "export" | "source" | "alias" | "if" | "then" | "fi" | "else" | "elif"
                | "for" | "do" | "done" | "case" | "esac" | "sudo" | "curl" | "cargo" | "git"
        ),
        "sql" => matches!(
            word.to_lowercase().as_str(),
            "select" | "from" | "where" | "insert" | "into" | "update" | "set" | "delete"
                | "join" | "left" | "right" | "inner" | "outer" | "on" | "group" | "by"
                | "order" | "having" | "limit" | "offset" | "create" | "table" | "drop"
                | "alter" | "index" | "primary" | "key" | "foreign" | "references" | "and"
                | "or" | "not" | "null" | "as"
        ),
        _ => matches!(
            word,
            "fn" | "let" | "pub" | "struct" | "class" | "def" | "function" | "const"
                | "return" | "if" | "else" | "import" | "from" | "async" | "await"
        ),
    }
}

fn is_boolean_or_number(word: &str) -> bool {
    matches!(word, "true" | "false" | "null" | "nil" | "None" | "True" | "False" | "undefined")
        || word.chars().all(|c| c.is_ascii_digit() || c == '.')
        || (word.starts_with("0x") && word[2..].chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_type_name(word: &str, _lang: &str) -> bool {
    matches!(
        word,
        "String" | "str" | "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32"
            | "i64" | "isize" | "f32" | "f64" | "bool" | "char" | "Option" | "Result"
            | "Some" | "None" | "Ok" | "Err" | "Vec" | "HashMap" | "HashSet" | "Arc"
            | "Rc" | "Box" | "Mutex" | "Self" | "Promise" | "Array" | "Object"
            | "Boolean" | "Number" | "int" | "float" | "dict" | "list" | "set" | "tuple"
    ) || (word.len() >= 2
        && word.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false)
        && word[1..].chars().any(|c| c.is_ascii_lowercase()))
}

pub fn parse_inline_spans(line: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = line.chars().collect();
    let mut current_text = String::new();

    while i < chars.len() {
        // Bold: **...** or __...__
        if (i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*')
            || (i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_')
        {
            let marker = chars[i];
            if !current_text.is_empty() {
                spans.push(InlineSpan::Text(current_text.clone()));
                current_text.clear();
            }
            i += 2;
            let start = i;
            let mut closed = false;
            while i + 1 < chars.len() {
                if chars[i] == marker && chars[i + 1] == marker {
                    let bold_content: String = chars[start..i].iter().collect();
                    if !bold_content.is_empty() {
                        spans.push(InlineSpan::Bold(bold_content));
                    }
                    i += 2;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                let remaining: String = chars[start..].iter().collect();
                current_text.push(marker);
                current_text.push(marker);
                current_text.push_str(&remaining);
                break;
            }
            continue;
        }

        // Inline code: `...`
        if chars[i] == '`' {
            if !current_text.is_empty() {
                spans.push(InlineSpan::Text(current_text.clone()));
                current_text.clear();
            }
            i += 1;
            let start = i;
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '`' {
                    let code_content: String = chars[start..i].iter().collect();
                    spans.push(InlineSpan::Code(code_content));
                    i += 1;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                let remaining: String = chars[start..].iter().collect();
                current_text.push('`');
                current_text.push_str(&remaining);
                break;
            }
            continue;
        }

        // Italic: *...*
        if chars[i] == '*' && (i + 1 >= chars.len() || chars[i + 1] != '*') {
            if !current_text.is_empty() {
                spans.push(InlineSpan::Text(current_text.clone()));
                current_text.clear();
            }
            i += 1;
            let start = i;
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '*' {
                    let italic_content: String = chars[start..i].iter().collect();
                    if !italic_content.is_empty() {
                        spans.push(InlineSpan::Italic(italic_content));
                    }
                    i += 1;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                let remaining: String = chars[start..].iter().collect();
                current_text.push('*');
                current_text.push_str(&remaining);
                break;
            }
            continue;
        }

        current_text.push(chars[i]);
        i += 1;
    }

    if !current_text.is_empty() {
        spans.push(InlineSpan::Text(current_text));
    }

    spans
}

pub fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    let sanitized = crate::infrastructure::utils::markdown_cleaner::clean_markdown_for_display(content);
    let mut blocks = Vec::new();
    let lines: Vec<&str> = sanitized.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // 1. Code Block: ```lang ... ```
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim().to_string();
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            let tokenized_lines: Vec<Vec<CodeToken>> = if lang != "diff" {
                code_lines.iter().map(|l| tokenize_code_line(l, &lang)).collect()
            } else {
                Vec::new()
            };
            blocks.push(MarkdownBlock::CodeBlock {
                lang,
                code: code_lines.join("\n"),
                tokenized_lines,
            });
            i += 1;
            continue;
        }

        // 2. Empty Line
        if trimmed.is_empty() {
            blocks.push(MarkdownBlock::EmptyLine);
            i += 1;
            continue;
        }

        // 3. Heading: #, ##, ###, ####, etc.
        if trimmed.starts_with('#') {
            let mut level = 0;
            for c in trimmed.chars() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }
            if level > 0 && level <= 6 {
                let header_text = trimmed[level..].trim().to_string();
                blocks.push(MarkdownBlock::Heading {
                    level,
                    text: header_text,
                });
                i += 1;
                continue;
            }
        }

        // 4. Bullet item: "- ", "* ", "• "
        if (trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("• "))
            && trimmed.len() >= 2
        {
            let item_text = if trimmed.starts_with("• ") {
                &trimmed[3..]
            } else {
                &trimmed[2..]
            };
            let spans = parse_inline_spans(item_text);
            blocks.push(MarkdownBlock::BulletItem(spans));
            i += 1;
            continue;
        }

        // 5. Numbered item: "1. ", "2. ", etc.
        let mut is_numbered = false;
        if let Some(dot_idx) = trimmed.find('.')
            && dot_idx > 0 && dot_idx <= 4 && trimmed[..dot_idx].chars().all(|c| c.is_ascii_digit())
                && let Some(rest) = trimmed[dot_idx + 1..].strip_prefix(' ') {
                    let num = trimmed[..dot_idx].to_string();
                    let spans = parse_inline_spans(rest.trim_start());
                    blocks.push(MarkdownBlock::NumberedItem {
                        number: num,
                        spans,
                    });
                    is_numbered = true;
                    i += 1;
                }
        if is_numbered {
            continue;
        }

        // 6. Regular Paragraph
        let spans = parse_inline_spans(trimmed);
        blocks.push(MarkdownBlock::Paragraph(spans));
        i += 1;
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_inline_spans_bold_and_code() {
        let text = "Here is **bold text** and `some_code()` alongside *italic*.";
        let spans = parse_inline_spans(text);

        assert!(spans.iter().any(|s| matches!(s, InlineSpan::Bold(b) if b == "bold text")));
        assert!(spans.iter().any(|s| matches!(s, InlineSpan::Code(c) if c == "some_code()")));
        assert!(spans.iter().any(|s| matches!(s, InlineSpan::Italic(i) if i == "italic")));
    }

    #[test]
    fn test_parse_markdown_blocks_headings() {
        let doc = "### Proposta de Arquitetura\nTexto explicativo.\n\n- Item 1\n- Item 2";
        let blocks = parse_markdown_blocks(doc);

        assert!(matches!(&blocks[0], MarkdownBlock::Heading { level: 3, text } if text == "Proposta de Arquitetura"));
        assert!(matches!(&blocks[1], MarkdownBlock::Paragraph(_)));
        assert!(matches!(&blocks[2], MarkdownBlock::EmptyLine));
        assert!(matches!(&blocks[3], MarkdownBlock::BulletItem(_)));
        assert!(matches!(&blocks[4], MarkdownBlock::BulletItem(_)));
    }

    #[test]
    fn test_tokenize_code_line_syntax_highlighting() {
        let line = "    pub async fn authenticate(name: &str) -> Result<User> { // check auth";
        let tokens = tokenize_code_line(line, "rust");

        assert!(tokens.iter().any(|t| matches!(t, CodeToken::Keyword(k) if k == "pub")));
        assert!(tokens.iter().any(|t| matches!(t, CodeToken::Keyword(k) if k == "async")));
        assert!(tokens.iter().any(|t| matches!(t, CodeToken::Keyword(k) if k == "fn")));
        assert!(tokens.iter().any(|t| matches!(t, CodeToken::Type(ty) if ty == "Result")));
        assert!(tokens.iter().any(|t| matches!(t, CodeToken::Type(ty) if ty == "User")));
        assert!(tokens.iter().any(|t| matches!(t, CodeToken::Comment(c) if c.contains("check auth"))));
    }

    #[test]
    fn test_parse_markdown_blocks_strips_patch_markers() {
        let doc = "Antes\n<<<<<<< SEARCH\nfoo\n=======\nbar\n>>>>>>> REPLACE\nDepois";
        let blocks = parse_markdown_blocks(doc);

        let rendered: Vec<String> = blocks
            .into_iter()
            .filter_map(|b| match b {
                MarkdownBlock::Paragraph(spans) => Some(
                    spans
                        .into_iter()
                        .map(|s| match s {
                            InlineSpan::Text(t) => t,
                            _ => String::new(),
                        })
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect();

        assert_eq!(rendered, vec!["Antes", "bar", "Depois"]);
    }
}
