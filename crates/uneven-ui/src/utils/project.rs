use crate::state::AppState;
use dioxus::prelude::*;

/// Opens the native OS file explorer to select a project directory and connects Uneven to it.
pub fn pick_and_open_project(mut state: Signal<AppState>) {
    spawn(async move {
        if let Some(folder) = rfd::AsyncFileDialog::new()
            .set_title("Open Project Directory")
            .pick_folder()
            .await
        {
            let path = folder.path().to_path_buf();
            state.write().open_project(path);
        }
    });
}

/// Opens the native OS file picker to select one or more files and attaches them to the prompt context.
pub fn pick_and_attach_files(mut state: Signal<AppState>) {
    spawn(async move {
        let mut dialog = rfd::AsyncFileDialog::new().set_title("Select Files to Attach to Context");
        let maybe_root = state.read().project_root.clone();
        if let Some(ref root) = maybe_root {
            dialog = dialog.set_directory(root);
        }
        if let Some(files) = dialog.pick_files().await {
            for file in files {
                let path = file.path();
                let rel = if let Some(ref root) = maybe_root {
                    path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string()
                } else {
                    path.to_string_lossy().to_string()
                };
                state.write().attach_file(rel);
            }
        }
    });
}
