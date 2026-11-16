# Uneven IDE Cockpit (`uneven-ui`)

![Version](https://img.shields.io/badge/version-0.2.0-cyan)
![Platform](https://img.shields.io/badge/platform-desktop%20(Linux%20%7C%20macOS%20%7C%20Windows)-blue)
![Framework](https://img.shields.io/badge/framework-Dioxus%200.7-orange)
![License](https://img.shields.io/badge/license-BSL%201.1-green)

The native visual desktop IDE for **Uneven Agent** — an autonomous senior AI software engineering and security auditor cockpit built with Rust and [Dioxus 0.7](https://dioxuslabs.com).

---

## Overview

Uneven IDE Cockpit provides a unified, zero-distraction developer control plane tailored for high-agency coding, architectural self-critique (`think`), automated context compaction, real-time token telemetry, multi-tab terminal workflows, and code editing.

```
crates/uneven-ui/
├── assets/         # Critical styles (tailwind.css, main.css), .desktop entry
├── icons/          # Multi-resolution transparent desktop crystal icons (32x32, 128x128, 256x256, icon.ico)
├── src/
│   ├── components/ # Prompt input, file tree, multi-tab editor, terminal tabs, diff viewers, status bar
│   ├── pages/      # Cockpit IDE workspace, Settings & Telemetry dashboard
│   ├── routes/     # Type-safe Dioxus application router
│   ├── state/      # Global reactive AppState and active sessions
│   ├── utils/      # Native window management, file dialogs (rfd), .env persistence
│   └── main.rs     # Desktop launcher with CLI project path resolution & zero FOUC
├── Cargo.toml      # Package manifest (v0.2.0) with desktop bundle metadata
└── Dioxus.toml     # Desktop bundle metadata and application icon mapping
```

---

## Key Features

### 1. 🖥️ Native OLED Desktop Experience & Packaging
- **Freedesktop & App Store Integration**: Properly packaged desktop entry displaying **Uneven IDE** across Linux app stores (GNOME Software, Ubuntu App Center), application launchers, and dockers.
- **Zero FOUC (Flash of Unstyled Content)**: Inlines critical background `#0a0a0a` and Tailwind styling directly into the native WebView head before first paint.
- **Auto-Monitor Resolution Detection**: Dynamically queries display hardware via `/sys/class/drm` and `xrandr` to maximize across your primary display.
- **Minimalist Crystal Icon**: Asymmetrical faceted crystal desktop icon with a 100% transparent alpha channel matching modern Linux and macOS docks.
- **CLI Project Launch**: Launch straight into your working directory from the terminal with `une .` or `uneven .`. Launching from the home directory (`~`) opens in clean welcome mode.

### 2. ⚡ Multi-Tab Integrated Terminal & Screen Expansion
- **Multi-Tab Workflows**: Run multiple independent shell sessions concurrently. Open new tabs with `+`, switch between tabs seamlessly, and terminate running sessions with per-tab `✕` controls.
- **Full-Screen Terminal Expansion (`⤢` / `⤡`)**: Maximize the terminal to occupy 100% of the cockpit height for heavy debugging, build logs, and long traces, or collapse back to split-view.
- **Interactive Control**: Dedicated process interruption (`Ctrl+C`), native clipboard integration (`Ctrl+Shift+C` / `Ctrl+Shift+V`), and automatic ANSI color parsing.

### 3. 💬 Conversational Engineering Shell
- **Smart Session Management**: Dynamically derived chat titles reflecting the topic of discussion instead of static placeholders.
- **Context Attachment & Native File Picker**: Attach files or folders directly into agent context via native OS dialogs (`rfd`) or the interactive workspace file tree.
- **Safe Token Guard**: 2,000-character prompt safety monitor with dynamic counter to prevent context overflow and encourage concise directives.
- **Autonomous Action Pods**: Real-time display of active AI clusters, multi-agent supervisory logs, and step-by-step reasoning steps.

### 4. 📂 Multi-Tab In-Place Code Editor
- **Selective Actions**: Seamlessly choose between viewing a file in the editor or attaching it to the prompt context.
- **Tabbed Navigation**: Open multiple workspace files simultaneously with dirty-state change tracking and smooth tab switching.
- **Direct Live Syntax Editing**: Click directly onto any line in the code viewer to make quick in-place modifications without losing syntax highlighting.
- **Focus Mode**: Automatically hides the conversational shell when a file tab is opened to maximize horizontal code viewing space.

### 5. ⚙️ Settings & Telemetry Dashboard
- **Resource Quotas & Consumption**:
  - **Work Turn (5h)**: Real-time sliding window usage percentage with countdown timer to reset.
  - **Weekly Window (7d)**: 7-day rolling window quota monitoring.
  - **Monthly Quota**: Billing cycle quota progress tracking.
  - **Efficiency Boost**: Quantified percentage of tokens saved via autonomous context compaction.
- **Secure Local Credential Management**:
  - Interactive expandable drawers for major AI brain providers: Google Gemini, Anthropic Claude, OpenAI, DeepSeek, Groq, Mistral, and local Ollama.
  - Database connectivity parameters (`DATABASE_URL`, `DB_API_KEY`) for autonomous schema analysis (`analyze`).
  - Secure local persistence without leaking filesystem paths to the UI.

---

## Launching via CLI

```bash
# Open Uneven IDE in the current directory
une .
# or
uneven .

# Open a specific project directory
une /path/to/project
# or
uneven ide /path/to/project

# Launch standalone without opening a project
uneven ide
```

---

## Building and Running

### Prerequisites
- Rust 2024 edition (1.85+ recommended)
- System WebView libraries (installed automatically by Linux package managers: `webkit2gtk-4.1` on Debian/Ubuntu, `webkit2gtk` on Arch/Fedora).

### Running in Development
```bash
# Run directly with Cargo
cargo run -p uneven-ui

# Or with Dioxus CLI (dx)
dx serve --platform desktop
```

### Running Unit Tests
```bash
cargo test -p uneven-ui --jobs 1
```

---

## License

Licensed under the **Business Source License 1.1 (BSL-1.1)**. See repository root for details.
