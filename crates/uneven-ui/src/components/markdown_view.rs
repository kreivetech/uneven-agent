use dioxus::prelude::*;
pub use uneven_core::domain::markdown::*;

#[component]
fn InlineSpansView(spans: Vec<InlineSpan>) -> Element {
    rsx! {
        for (idx, span) in spans.into_iter().enumerate() {
            match span {
                InlineSpan::Text(s) => rsx! {
                    span { key: "{idx}", "{s}" }
                },
                InlineSpan::Bold(s) => rsx! {
                    strong { key: "{idx}", class: "font-semibold text-neutral-100", "{s}" }
                },
                InlineSpan::Code(s) => rsx! {
                    code {
                        key: "{idx}",
                        class: "px-1.5 py-0.5 mx-0.5 rounded-md bg-neutral-800/90 text-[12px] font-mono text-cyan-300 border border-neutral-700/60 select-text font-medium",
                        "{s}"
                    }
                },
                InlineSpan::Italic(s) => rsx! {
                    em { key: "{idx}", class: "italic text-neutral-300", "{s}" }
                },
            }
        }
    }
}

#[component]
fn HighlightedCodeView(tokenized_lines: Vec<Vec<CodeToken>>) -> Element {
    rsx! {
        code { class: "block font-mono text-xs leading-relaxed select-text",
            for (line_idx, tokens) in tokenized_lines.into_iter().enumerate() {
                div { key: "cl_{line_idx}", class: "table-row hover:bg-white/[0.02]",
                    span { class: "table-cell select-none pr-4 text-right text-neutral-600 text-[11px] w-8 font-mono",
                        "{line_idx + 1}"
                    }
                    span { class: "table-cell whitespace-pre font-mono",
                        for (tok_idx, token) in tokens.into_iter().enumerate() {
                            match token {
                                CodeToken::Keyword(s) => rsx! {
                                    span { key: "t_{tok_idx}", class: "text-cyan-400 font-semibold", "{s}" }
                                },
                                CodeToken::Type(s) => rsx! {
                                    span { key: "t_{tok_idx}", class: "text-sky-300 font-medium", "{s}" }
                                },
                                CodeToken::StringLiteral(s) => rsx! {
                                    span { key: "t_{tok_idx}", class: "text-emerald-300", "{s}" }
                                },
                                CodeToken::Comment(s) => rsx! {
                                    span { key: "t_{tok_idx}", class: "text-neutral-500 italic", "{s}" }
                                },
                                CodeToken::Number(s) => rsx! {
                                    span { key: "t_{tok_idx}", class: "text-amber-300", "{s}" }
                                },
                                CodeToken::Plain(s) => rsx! {
                                    span { key: "t_{tok_idx}", class: "text-neutral-200", "{s}" }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn MarkdownView(
    #[props(default)] blocks: Option<Vec<MarkdownBlock>>,
    #[props(default)] content: Option<String>,
) -> Element {
    let rendered_blocks = if let Some(b) = blocks {
        b
    } else if let Some(ref c) = content {
        parse_markdown_blocks(c)
    } else {
        Vec::new()
    };

    rsx! {
        div { class: "flex flex-col space-y-1 font-sans text-xs sm:text-sm text-neutral-200 leading-relaxed select-text",
            for (idx, block) in rendered_blocks.into_iter().enumerate() {
                match block {
                    MarkdownBlock::Heading { level, text } => match level {
                        1 => rsx! {
                            h2 {
                                key: "h_{idx}",
                                class: "text-base font-bold text-cyan-400 mt-4 mb-2 tracking-tight font-sans border-b border-neutral-800/80 pb-1",
                                "{text}"
                            }
                        },
                        2 => rsx! {
                            h3 {
                                key: "h_{idx}",
                                class: "text-sm font-semibold text-cyan-400 mt-3 mb-1.5 tracking-tight font-sans",
                                "{text}"
                            }
                        },
                        3 => rsx! {
                            h4 {
                                key: "h_{idx}",
                                class: "text-[13px] font-medium text-cyan-300 mt-2.5 mb-1 tracking-tight font-sans",
                                "{text}"
                            }
                        },
                        _ => rsx! {
                            h5 {
                                key: "h_{idx}",
                                class: "text-xs font-medium text-cyan-200/90 mt-2 mb-0.5 tracking-tight font-sans",
                                "{text}"
                            }
                        },
                    },
                    MarkdownBlock::Paragraph(spans) => rsx! {
                        p { key: "p_{idx}", class: "my-0.5 leading-relaxed",
                            InlineSpansView { spans }
                        }
                    },
                    MarkdownBlock::BulletItem(spans) => rsx! {
                        div { key: "b_{idx}", class: "flex items-start gap-2 my-0.5 pl-1 leading-relaxed",
                            span { class: "text-cyan-400/80 select-none text-xs mt-0.5", "•" }
                            div { class: "flex-1",
                                InlineSpansView { spans }
                            }
                        }
                    },
                    MarkdownBlock::NumberedItem { number, spans } => rsx! {
                        div { key: "n_{idx}", class: "flex items-start gap-2 my-0.5 pl-1 leading-relaxed",
                            span { class: "text-cyan-400/90 font-mono text-xs select-none min-w-[18px]", "{number}." }
                            div { class: "flex-1",
                                InlineSpansView { spans }
                            }
                        }
                    },
                    MarkdownBlock::CodeBlock { lang, code, tokenized_lines } => rsx! {
                        div {
                            key: "c_{idx}",
                            class: "my-3 rounded-xl bg-neutral-950 border border-neutral-800/90 overflow-hidden select-text shadow-md",
                            if !lang.is_empty() {
                                div { class: "px-3.5 py-1.5 bg-neutral-900/90 border-b border-neutral-800/70 flex items-center justify-between text-[11px] font-mono text-cyan-400 uppercase tracking-wider select-none font-semibold",
                                    div { class: "flex items-center gap-1.5",
                                        span { class: "w-2 h-2 rounded-full bg-cyan-400/80" }
                                        span { "{lang}" }
                                    }
                                }
                            }
                            pre { class: "p-3.5 overflow-x-auto text-xs font-mono leading-relaxed scrollbar-thin",
                                if lang == "diff" {
                                    for (l_idx, line) in code.lines().enumerate() {
                                        if line.starts_with('+') {
                                            div { key: "dl_{l_idx}", class: "text-emerald-400 bg-emerald-500/10 px-1 -mx-1 rounded-sm", "{line}" }
                                        } else if line.starts_with('-') {
                                            div { key: "dl_{l_idx}", class: "text-rose-400 bg-rose-500/10 px-1 -mx-1 rounded-sm", "{line}" }
                                        } else if line.starts_with("@@") {
                                            div { key: "dl_{l_idx}", class: "text-cyan-400 font-semibold my-0.5", "{line}" }
                                        } else {
                                            div { key: "dl_{l_idx}", class: "text-neutral-400", "{line}" }
                                        }
                                    }
                                } else {
                                    HighlightedCodeView { tokenized_lines: tokenized_lines.clone() }
                                }
                            }
                        }
                    },
                    MarkdownBlock::EmptyLine => rsx! {
                        div { key: "e_{idx}", class: "h-1" }
                    },
                }
            }
        }
    }
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
