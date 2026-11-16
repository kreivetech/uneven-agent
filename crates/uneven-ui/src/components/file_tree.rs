#![allow(dead_code)]
use crate::state::AppState;
use dioxus::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<FileEntry>,
}

fn scan_single_dir(dir: &Path) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Ignore hidden and heavy build directories
            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "dist"
                || name == "obsolete"
                || name == "build"
                || name == ".git"
            {
                continue;
            }

            let path = entry.path();
            let is_dir = path.is_dir();

            entries.push(FileEntry {
                name,
                path,
                is_dir,
                children: Vec::new(),
            });
        }
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    entries
}

#[component]
fn FileNode(entry: FileEntry, depth: usize) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut is_open = use_signal(|| false);
    let mut children = use_signal(Vec::<FileEntry>::new);
    let mut is_loaded = use_signal(|| false);
    let padding_left = format!("{}px", (depth * 10) + 6);

    let rel_path = if let Some(ref root) = state.read().project_root {
        entry
            .path
            .strip_prefix(root)
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .to_string()
    } else {
        entry.name.clone()
    };

    let is_active_tab = state
        .read()
        .active_file_tab()
        .map(|v| v.rel_path == rel_path)
        .unwrap_or(false);
    let is_open_in_tab = state
        .read()
        .open_tabs
        .iter()
        .any(|v| v.rel_path == rel_path);
    let is_attached = !entry.is_dir && state.read().attached_files.contains(&rel_path);

    let rel_click = rel_path.clone();
    let rel_attach = rel_path.clone();
    let dir_path_to_scan = entry.path.clone();

    rsx! {
        div { class: "flex flex-col select-none",
            div {
                class: if is_active_tab {
                    "group flex items-center gap-1.5 py-1 px-1.5 rounded bg-cyan-950/60 border border-cyan-700/60 text-white cursor-pointer text-[11px] font-mono transition-colors shadow-sm"
                } else if is_open_in_tab {
                    "group flex items-center gap-1.5 py-1 px-1.5 rounded bg-neutral-900 border border-neutral-700 text-neutral-200 cursor-pointer text-[11px] font-mono transition-colors"
                } else if is_attached {
                    "group flex items-center gap-1.5 py-1 px-1.5 rounded bg-neutral-900/60 border border-neutral-800 text-cyan-300 cursor-pointer text-[11px] font-mono transition-colors"
                } else {
                    "group flex items-center gap-1.5 py-1 px-1.5 rounded hover:bg-neutral-800/60 text-neutral-300 hover:text-white cursor-pointer text-[11px] font-mono transition-colors"
                },
                style: "padding-left: {padding_left};",
                title: if entry.is_dir {
                    "Toggle folder"
                } else {
                    "Click to view/edit file in tabs"
                },
                onclick: move |_| {
                    if entry.is_dir {
                        let next_open = !*is_open.read();
                        if next_open && !*is_loaded.read() {
                            let sub = scan_single_dir(&dir_path_to_scan);
                            children.set(sub);
                            is_loaded.set(true);
                        }
                        is_open.set(next_open);
                    } else {
                        state.write().open_file_tab(rel_click.clone());
                    }
                },

                // Sleek Monochromatic Icons (No yellow emojis!)
                if entry.is_dir {
                    svg {
                        class: if *is_open.read() {
                            "w-3.5 h-3.5 text-neutral-300 shrink-0"
                        } else {
                            "w-3.5 h-3.5 text-neutral-500 group-hover:text-neutral-300 shrink-0"
                        },
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "1.75",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z",
                        }
                    }
                } else if entry.name.ends_with(".rs") {
                    span { class: "text-[9px] font-mono font-bold text-cyan-400 shrink-0 px-0.5 rounded bg-cyan-950/50 border border-cyan-800/40", "rs" }
                } else if entry.name.ends_with(".toml") {
                    span { class: "text-[9px] font-mono font-bold text-amber-300 shrink-0 px-0.5 rounded bg-amber-950/40 border border-amber-800/40", "cfg" }
                } else if entry.name.ends_with(".json") {
                    span { class: "text-[9px] font-mono font-bold text-emerald-400 shrink-0 px-0.5 rounded bg-emerald-950/40 border border-emerald-800/40", "json" }
                } else if entry.name.ends_with(".md") {
                    span { class: "text-[9px] font-mono font-bold text-blue-400 shrink-0 px-0.5 rounded bg-blue-950/40 border border-blue-800/40", "md" }
                } else if entry.name.ends_with(".css") || entry.name.ends_with(".html") {
                    span { class: "text-[9px] font-mono font-bold text-indigo-400 shrink-0 px-0.5 rounded bg-indigo-950/40 border border-indigo-800/40", "web" }
                } else {
                    svg {
                        class: "w-3 h-3 text-neutral-500 group-hover:text-neutral-300 shrink-0",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "1.75",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z",
                        }
                    }
                }

                span { class: "truncate flex-1 text-[11px]", "{entry.name}" }

                if entry.is_dir {
                    span { class: "text-[8px] text-neutral-600 group-hover:text-neutral-400 ml-auto font-mono",
                        if *is_open.read() { "▼" } else { "▶" }
                    }
                } else {
                    button {
                        class: if is_attached {
                            "text-[10px] text-cyan-400 p-0.5 rounded hover:bg-cyan-900/40 ml-auto cursor-pointer"
                        } else {
                            "text-[10px] text-neutral-500 hover:text-white opacity-0 group-hover:opacity-100 transition-opacity p-0.5 rounded hover:bg-neutral-700 ml-auto cursor-pointer"
                        },
                        title: if is_attached { "Remove from prompt context" } else { "Attach to prompt context (+)" },
                        onclick: move |e| {
                            e.stop_propagation();
                            if is_attached {
                                let pos = state.read().attached_files.iter().position(|f| f == &rel_attach);
                                if let Some(p) = pos {
                                    state.write().remove_attached_file(p);
                                }
                            } else {
                                state.write().attach_file(rel_attach.clone());
                            }
                        },
                        if is_attached { "✓" } else { "+" }
                    }
                }
            }

            if entry.is_dir && *is_open.read() {
                div { class: "flex flex-col",
                    for child in children.read().iter() {
                        FileNode { key: "{child.path.display()}", entry: child.clone(), depth: depth + 1 }
                    }
                }
            }
        }
    }
}

#[component]
pub fn FileTree() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current_state = state.read();
    let maybe_root = current_state.project_root.clone();

    let root = match maybe_root {
        Some(r) => r,
        None => {
            return rsx! {
                div { class: "p-4 text-center text-xs text-neutral-500 font-sans",
                    "Select a project to explore files."
                }
            };
        }
    };

    let files = use_memo(move || scan_single_dir(&root));

    rsx! {
        div { class: "flex flex-col flex-1 overflow-y-auto overflow-x-hidden min-h-0 select-none",
            div { class: "px-2 py-1 text-[10px] font-semibold tracking-wider text-neutral-500 uppercase flex items-center justify-between border-b border-[#1c1c1c] mb-1",
                span { "Explorer" }
                span { class: "text-[9px] text-neutral-500 font-mono", "{files.read().len()} items" }
            }
            div { class: "flex flex-col py-0.5",
                for entry in files.read().iter() {
                    FileNode { key: "{entry.path.display()}", entry: entry.clone(), depth: 0 }
                }
            }
        }
    }
}
