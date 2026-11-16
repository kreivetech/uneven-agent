//! Native Git operations using libgit2.
//!
//! Provides high-performance access to repository history, diffs, and status.

use crate::UnevenError;
use git2::{Repository, DiffOptions, StatusOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitCommit {
    pub id: String,
    pub author: String,
    pub message: String,
    pub time: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitDiff {
    pub files: Vec<String>,
    pub patch: String,
}

/// Get the commit history of a repository.
pub fn get_history(repo_path: &str, limit: usize) -> Result<Vec<GitCommit>, UnevenError> {
    let repo = Repository::discover(repo_path)
        .map_err(|e| UnevenError::GitError(format!("Failed to find repo: {}", e)))?;
    
    let mut revwalk = repo.revwalk()
        .map_err(|e| UnevenError::GitError(format!("Failed to create revwalk: {}", e)))?;
    
    revwalk.push_head()
        .map_err(|e| UnevenError::GitError(format!("Failed to push HEAD to revwalk: {}", e)))?;
    
    let commits = revwalk
        .take(limit)
        .filter_map(|id| {
            let id = id.ok()?;
            let commit = repo.find_commit(id).ok()?;
            let author = commit.author().name().ok()?.to_string();
            let message = commit.message().ok()?.trim().to_string();
            
            Some(GitCommit {
                id: id.to_string(),
                author,
                message,
                time: commit.time().seconds(),
            })
        })
        .collect();
    
    Ok(commits)
}

/// Get the diff of staged changes.
pub fn get_staged_diff(repo_path: &str) -> Result<GitDiff, UnevenError> {
    let repo = Repository::discover(repo_path)
        .map_err(|e| UnevenError::GitError(format!("Failed to find repo: {}", e)))?;
    
    let head = repo.head()
        .and_then(|h| h.peel_to_tree())
        .ok(); // Might be an empty repo
    
    let mut opts = DiffOptions::new();
    let diff = repo.diff_tree_to_index(head.as_ref(), None, Some(&mut opts))
        .map_err(|e| UnevenError::GitError(format!("Failed to generate diff: {}", e)))?;
    
    let mut files = Vec::new();
    let mut patch = String::new();
    
    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        if let Some(path) = delta.new_file().path().and_then(|p| p.to_str())
            && !files.contains(&path.to_string()) {
                files.push(path.to_string());
            }
        
        let origin = line.origin();
        if origin == '+' || origin == '-' || origin == ' ' {
            patch.push(origin);
        }
        
        if let Ok(content) = std::str::from_utf8(line.content()) {
            patch.push_str(content);
        }
        true
    }).map_err(|e| UnevenError::GitError(format!("Failed to format diff: {}", e)))?;
    
    Ok(GitDiff { files, patch })
}

/// Get the list of modified (unstaged) files.
pub fn get_status(repo_path: &str) -> Result<Vec<String>, UnevenError> {
    let repo = Repository::discover(repo_path)
        .map_err(|e| UnevenError::GitError(format!("Failed to find repo: {}", e)))?;
    
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    
    let statuses = repo.statuses(Some(&mut opts))
        .map_err(|e| UnevenError::GitError(format!("Failed to get statuses: {}", e)))?;
    
    let files = statuses.iter()
        .filter_map(|entry| entry.path().ok().map(|p| p.to_string()))
        .collect();
    
    Ok(files)
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FixCommit {
    pub hash: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitUndoResult {
    pub reverted_commit: String,
    pub revert_commit: String,
    pub branch: String,
}

pub fn list_fix_commits(repo_path: &str, limit: usize) -> Result<Vec<FixCommit>, UnevenError> {
    let repo = Repository::discover(repo_path)
        .map_err(|e| UnevenError::GitError(format!("Failed to find repo: {}", e)))?;
    
    let mut revwalk = repo.revwalk()
        .map_err(|e| UnevenError::GitError(format!("Failed to create revwalk: {}", e)))?;
    
    revwalk.push_head()
        .map_err(|e| UnevenError::GitError(format!("Failed to push HEAD to revwalk: {}", e)))?;
    
    let mut commits = Vec::new();
    for id in revwalk {
        let id = match id {
            Ok(x) => x,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(id) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let message = commit.message().unwrap_or("").trim();
        if message.contains("fix(uneven):") {
            let time = commit.time();
            let datetime = chrono::DateTime::from_timestamp(time.seconds(), 0)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %z").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            commits.push(FixCommit {
                hash: id.to_string(),
                date: datetime,
                message: message.to_string(),
            });
            if commits.len() >= limit {
                break;
            }
        }
    }
    
    Ok(commits)
}

pub fn revert_commit(repo_path: &str, commit_hash: &str) -> Result<GitUndoResult, UnevenError> {
    let output = std::process::Command::new("git")
        .args(["revert", "--no-edit", commit_hash])
        .current_dir(repo_path)
        .output()
        .map_err(|e| UnevenError::GitError(format!("Failed to execute git revert: {}", e)))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(UnevenError::GitError(format!("Git revert failed: {}", err_msg)));
    }

    let branch_output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| UnevenError::GitError(format!("Failed to get current branch: {}", e)))?;
    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

    let head_output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| UnevenError::GitError(format!("Failed to get HEAD: {}", e)))?;
    let revert_commit = String::from_utf8_lossy(&head_output.stdout).trim().to_string();

    Ok(GitUndoResult {
        reverted_commit: commit_hash.to_string(),
        revert_commit,
        branch,
    })
}

pub fn undo_last_fix(repo_path: &str) -> Result<GitUndoResult, UnevenError> {
    let repo = Repository::discover(repo_path)
        .map_err(|e| UnevenError::GitError(format!("Failed to find repo: {}", e)))?;
    let head = repo.head()
        .map_err(|e| UnevenError::GitError(format!("Failed to get HEAD ref: {}", e)))?;
    let commit = head.peel_to_commit()
        .map_err(|e| UnevenError::GitError(format!("Failed to peel to commit: {}", e)))?;
    
    let last_hash = commit.id().to_string();
    let last_message = commit.message().unwrap_or("").trim().to_string();

    if !last_message.contains("fix(uneven):") {
        return Err(UnevenError::GitError(format!(
            "[UE-05000005] Last commit does not appear to be an Uneven fix:\n\"{}\"\nUse 'git revert' manually if you still want to undo it.",
            if last_message.len() > 80 { &last_message[..80] } else { &last_message }
        )));
    }

    revert_commit(repo_path, &last_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_get_history() {
        let repo_path = env::current_dir().unwrap();
        let repo_str = repo_path.to_str().unwrap();
        let history = get_history(repo_str, 5).unwrap();
        assert!(!history.is_empty());
        println!("Last commit: {}", history[0].message);
    }

    #[test]
    fn test_get_status() {
        let repo_path = env::current_dir().unwrap();
        let repo_str = repo_path.to_str().unwrap();
        let status = get_status(repo_str).unwrap();
        // Since we are editing files, status should likely not be empty
        println!("Modified files: {:?}", status);
    }
}

