use console::style;
use std::io::Write;

// ─── Cores do Tema (Tokens de Design do Uneven) ───────────────────────────────

pub fn accent(s: &str) -> String {
    truecolor(s, 168, 200, 255) // #a8c8ff - azul gelo / acento primário
}

pub fn bright(s: &str) -> String {
    truecolor(s, 208, 232, 255) // #d0e8ff - destaque brilhante
}

pub fn crystal(s: &str) -> String {
    truecolor(s, 232, 237, 245) // #e8edf5 - texto primário
}

pub fn dim(s: &str) -> String {
    truecolor(s, 107, 122, 150) // #6b7a96 - cinza muted / descrições
}

pub fn green(s: &str) -> String {
    truecolor(s, 0, 255, 140) // #00ff8c - sucesso
}

pub fn rust(s: &str) -> String {
    truecolor(s, 255, 107, 53) // #ff6b35 - avisos / erros
}

#[allow(dead_code)]
pub fn bold(s: &str) -> String {
    truecolor_bold(s, 232, 237, 245)
}

pub fn accent_bold(s: &str) -> String {
    truecolor_bold(s, 168, 200, 255)
}

// ─── Utilitários de Alinhamento ───────────────────────────────────────────────

pub fn pad(str: &str, total: usize) -> String {
    let clean = console::strip_ansi_codes(str);
    let len = clean.chars().count();
    if total > len {
        format!("{}{}", str, " ".repeat(total - len))
    } else {
        str.to_string()
    }
}

#[allow(dead_code)]
pub fn center(str: &str, total: usize) -> String {
    let clean = console::strip_ansi_codes(str);
    let len = clean.chars().count();
    if total > len {
        let left = (total - len) / 2;
        let right = total - len - left;
        format!("{}{}{}", " ".repeat(left), str, " ".repeat(right))
    } else {
        str.to_string()
    }
}

// ─── Componentes Visuais do CLI ───────────────────────────────────────────────

#[allow(dead_code)]
const W: usize = 52; // Largura interna do box de diálogo

pub fn box_header(version: &str) {
    let line1 = format!("◈  𝗨 𝗡 𝗘 𝗩 𝗘 𝗡   𝗔 𝗚 𝗘 𝗡 𝗧   v{}  ◈", version);
    let line2 = "autonomous development agent";
    println!();
    println!("  {}", bright(&line1));
    println!("  {}", dim(line2));
    println!();
}

pub fn box_welcome(version: &str, brain: &str, is_first_run: bool) {
    let line1 = format!("◈  𝗨 𝗡 𝗘 𝗩 𝗘 𝗡   𝗔 𝗚 𝗘 𝗡 𝗧   v{}  ◈", version);
    let line2 = "Person: (Senior Engineer)";

    println!("  {}", bright(&line1));
    println!("  {}", dim(line2));
    if is_first_run {
        let line3 = "Description: Autonomous local development agent.";
        println!("  {}", dim(line3));
    } else {
        let line3 = format!("Brain: {}", brain);
        println!("  {}", dim(&line3));
    }
    println!();
}

pub fn section(title: &str) {
    println!();
    println!("{}", accent_bold(&format!("  ◈  {}", title)));
    println!();
}

pub fn ok(msg: &str) {
    println!("{} {}", green("  ✓  "), crystal(msg));
}

#[allow(dead_code)]
pub fn fail(msg: &str) {
    println!("{} {}", rust("  ✗  "), crystal(msg));
}

pub fn warn(msg: &str) {
    println!("{} {}", rust("  ⚠  "), crystal(msg));
}

pub fn info(msg: &str) {
    println!("{} {}", accent("  ›  "), dim(msg));
}

pub fn hint(msg: &str) {
    println!("     {}", dim(msg));
}

pub fn blank() {
    println!();
}

pub fn cmd_line(cmd: &str, desc: &str) {
    let padded = pad(cmd, 24);
    println!("     {} {}", accent(&padded), dim(desc));
}

// ─── Funções Legadas (Compatibilidade) ────────────────────────────────────────

#[allow(dead_code)]
pub fn print_info(msg: &str) {
    println!("{}", style(msg).cyan());
}

#[allow(dead_code)]
pub fn print_success(msg: &str) {
    println!("{} {}", style("✓").green().bold(), msg);
}

#[allow(dead_code)]
pub fn print_warning(msg: &str) {
    println!("{} {}", style("⚠").yellow().bold(), style(msg).yellow());
}

pub fn truecolor(text: &str, r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text)
}

pub fn truecolor_bold(text: &str, r: u8, g: u8, b: u8) -> String {
    format!("\x1b[1;38;2;{};{};{}m{}\x1b[0m", r, g, b, text)
}

pub struct StreamRenderer {
    prefix: String,
}

impl StreamRenderer {
    pub fn new() -> Self {
        Self {
            prefix: "  ".to_string(),
        }
    }

    pub fn push(&mut self, token: &str) {
        for c in token.chars() {
            if c == '\n' {
                print!("\n{}", self.prefix);
            } else {
                print!("{}", crystal(&c.to_string()));
            }
        }
        let _ = std::io::stdout().flush();
    }
}

pub fn select_item(title: &str, options: &[&str]) -> Option<usize> {
    use console::{Key, Term};
    let term = Term::stdout();
    let mut selected = 0;
    let _ = term.hide_cursor();

    loop {
        println!("  {}", accent(title));
        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                println!("    {} {}", green("➔"), bright(opt));
            } else {
                println!("      {}", dim(opt));
            }
        }

        let key = term.read_key().unwrap_or(Key::Unknown);
        match key {
            Key::ArrowUp if selected > 0 => {
                selected -= 1;
            }
            Key::ArrowDown if selected < options.len() - 1 => {
                selected += 1;
            }
            Key::Enter => {
                let _ = term.show_cursor();
                let _ = term.clear_last_lines(options.len() + 1);
                return Some(selected);
            }
            Key::Escape => {
                let _ = term.show_cursor();
                let _ = term.clear_last_lines(options.len() + 1);
                return None;
            }
            _ => {}
        }

        // Clear printed options + title line to redraw
        let _ = term.clear_last_lines(options.len() + 1);
    }
}

pub struct TerminalMarkdownFormatter {
    pub bold_active: bool,
    pub pending_star: bool,
    pub heading_active: bool,
    pub line_start: bool,
    pub pending_hashes: usize,
}

impl TerminalMarkdownFormatter {
    pub fn new() -> Self {
        Self {
            bold_active: false,
            pending_star: false,
            heading_active: false,
            line_start: true,
            pending_hashes: 0,
        }
    }

    pub fn process_token(&mut self, token: &str) -> String {
        let mut result = String::new();
        let mut chars = token.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\n' {
                if self.heading_active {
                    result.push_str("\x1b[0m");
                    self.heading_active = false;
                }
                result.push('\n');
                self.line_start = true;
                self.pending_hashes = 0;
                continue;
            }

            if self.line_start {
                if c == '#' {
                    self.pending_hashes += 1;
                    continue;
                } else if c == ' ' && self.pending_hashes > 0 {
                    self.heading_active = true;
                    self.line_start = false;
                    self.pending_hashes = 0;
                    result.push_str("\x1b[1;36m"); // Cyan Bold
                    continue;
                } else {
                    for _ in 0..self.pending_hashes {
                        result.push('#');
                    }
                    self.pending_hashes = 0;
                    self.line_start = false;
                }
            }

            if c == '*' {
                if self.pending_star {
                    self.bold_active = !self.bold_active;
                    self.pending_star = false;
                    if self.bold_active {
                        result.push_str("\x1b[1m");
                    } else {
                        result.push_str("\x1b[22m");
                    }
                } else {
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        self.bold_active = !self.bold_active;
                        if self.bold_active {
                            result.push_str("\x1b[1m");
                        } else {
                            result.push_str("\x1b[22m");
                        }
                    } else {
                        self.pending_star = true;
                    }
                }
            } else {
                if self.pending_star {
                    result.push('*');
                    self.pending_star = false;
                }
                result.push(c);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_markdown_formatter_bold() {
        let mut formatter = TerminalMarkdownFormatter::new();

        // Single token bold
        assert_eq!(
            formatter.process_token("This is **bold** text"),
            "This is \x1b[1mbold\x1b[22m text"
        );

        // Bold split across tokens
        let mut formatter2 = TerminalMarkdownFormatter::new();
        assert_eq!(formatter2.process_token("This is **"), "This is \x1b[1m");
        assert_eq!(formatter2.process_token("bold** text"), "bold\x1b[22m text");
    }

    #[test]
    fn test_terminal_markdown_formatter_heading() {
        let mut formatter = TerminalMarkdownFormatter::new();

        // Heading detection
        assert_eq!(
            formatter.process_token("## Heading Title"),
            "\x1b[1;36mHeading Title"
        );

        // Heading reset on newline
        assert_eq!(
            formatter.process_token("\nNormal text"),
            "\x1b[0m\nNormal text"
        );
    }
}
