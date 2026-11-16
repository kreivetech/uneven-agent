# Custom Skills and Workspace Guidelines in Uneven Agent

Uneven Agent allows you to customize the behavior, rules, and intelligence of the AI brain at the workspace level. You can configure these by defining custom directory paths for skills, custom personas, or adding workspace-level guidelines.

---

## 1. Custom Workspace Skills

You can configure a custom skills folder using the `/skills` slash command inside the interactive shell, or by adding the `"skills"` key to your `config.json`. 

Uneven Agent recursively scans the configured skills folder and appends the markdown instructions to the AI system context. Habilities (skills) can be structured in two ways:

### Format A: Flat Markdown Files
Any `.md` file located directly in the skills folder is loaded. The file name is used as the skill name.
* **Path Example:** `<skills_dir>/deploy-helper.md`
* **Output in prompt:**
  ```markdown
  --- Skill: deploy-helper ---
  <contents of deploy-helper.md>
  ```

### Format B: Structured Subdirectories
If you have a complex skill requiring supporting scripts, examples, or references, you can organize it in a subdirectory containing a `SKILL.md` file. Uneven Agent will load `SKILL.md` and ignore helper folders during prompt assembly.
* **Path Example:** `<skills_dir>/git-manager/SKILL.md`
* **Output in prompt:**
  ```markdown
  --- Skill: git-manager ---
  <contents of SKILL.md>
  ```

---

## 2. Workspace Guidelines (`AGENTS.md` / `rules.md`)

To enforce coding styles, formatting preferences, or security rules globally across the workspace, you can create a guidelines markdown file in your project root. Uneven Agent automatically checks for these files and appends their contents to the active system instructions under the `=== ADDITIONAL WORKSPACE GUIDELINES ===` block.

The engine searches for files at the following locations (the first one found is loaded):
1. `<project_root>/.uneven/rules.md`
2. `<project_root>/.agents/AGENTS.md`

---

## 3. Custom Agent Persona

You can modify the identity and behavior of the agent by defining a custom persona using the `/person` slash command or the `"persona"` key in `config.json`.
* **Default Persona:** *"You are Snatchy, a senior software engineer and QA specialist. Your goal is to write clean, compile-ready code..."*
* **Custom Persona:** You can replace this identity entirely (e.g., *"You are a database tuning administrator focused on query indexing optimizations..."*). This custom description is prepended at the very beginning of the LLM context wrapper.
