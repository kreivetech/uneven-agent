use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_files(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, base, out);
        } else if path.is_file()
            && let Ok(rel) = path.strip_prefix(base)
        {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            out.push((rel_str, path));
        }
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let skills_dir = manifest_dir.join("assets").join("skills");
    println!("cargo:rerun-if-changed={}", skills_dir.display());

    let mut files = Vec::new();
    collect_files(&skills_dir, &skills_dir, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut generated = String::from("pub const DEFAULT_SKILL_FILES: &[(&str, &str)] = &[\n");
    for (rel_path, abs_path) in &files {
        println!("cargo:rerun-if-changed={}", abs_path.display());
        let abs_str = abs_path.to_string_lossy().replace('\\', "/");
        generated.push_str(&format!(
            "    ({:?}, include_str!({:?})),\n",
            rel_path, abs_str
        ));
    }
    generated.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("default_skills.rs"), generated).unwrap();
}
