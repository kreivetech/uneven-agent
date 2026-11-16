use std::path::Path;
use std::fs;
use crate::infrastructure::security::laws;

const MARKER_START: &str = "<!-- UNEVEN_AI_PROTOCOLS_START -->";
const MARKER_END: &str = "<!-- UNEVEN_AI_PROTOCOLS_END -->";

struct AgentConfig {
    folder: &'static str,
    file: &'static str,
}

const AGENTS: &[AgentConfig] = &[
    AgentConfig { folder: ".cursor",  file: ".cursorrules" },
    AgentConfig { folder: ".windsurf", file: ".windsurfrules" },
    AgentConfig { folder: ".claude",   file: ".claude/instructions.md" },
    AgentConfig { folder: ".github",   file: ".github/copilot-instructions.md" },
];

pub fn sync_agent_protocols(project_root: &str) -> Vec<String> {
    let protocols = laws::get_agent_protocols();
    let mut synced = Vec::new();
    let formatted = format!("\n{}\n\n{}\n\n{}\n", MARKER_START, protocols, MARKER_END);

    for agent in AGENTS {
        let folder_path = Path::new(project_root).join(agent.folder);
        let file_path = Path::new(project_root).join(agent.file);

        // RULE: Only inject if agent directory exists
        if folder_path.exists() && folder_path.is_dir() {
            // SECURITY: Never inject into the .uneven folder
            if agent.file.contains(".uneven") {
                continue;
            }

            if inject_to_file(&file_path, &formatted).is_ok() {
                synced.push(agent.file.to_string());
            }
        }
    }

    synced
}

fn inject_to_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let current_content = if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };

    let new_content = if current_content.contains(MARKER_START) {
        // Atualiza bloco existente
        let start_idx = current_content.find(MARKER_START).unwrap();
        let end_idx = current_content.find(MARKER_END).unwrap() + MARKER_END.len();
        let mut c = current_content.clone();
        c.replace_range(start_idx..end_idx, content.trim());
        c
    } else {
        // Adiciona novo bloco
        format!("{}\n\n{}", current_content.trim(), content)
    };

    fs::write(path, new_content.trim())?;
    Ok(())
}
