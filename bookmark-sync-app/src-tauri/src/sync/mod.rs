pub mod credentials;

use git2::{Repository, Signature, Cred};
use std::path::Path;

/// Clones or opens the local repository to store event logs
pub fn init_or_open_repo(app_data_dir: &Path, repo_url: &str, token: &str) -> Result<Repository, String> {
    let repo_path = app_data_dir.join("sync-repo");

    if repo_path.exists() {
        // Open existing
        Repository::open(&repo_path).map_err(|e| e.to_string())
    } else {
        // Clone new
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::userpass_plaintext(
                username_from_url.unwrap_or("git"),
                token,
            )
        });

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);

        builder.clone(repo_url, &repo_path).map_err(|e| e.to_string())
    }
}

/// Commits all modified and untracked files in the repository
pub fn commit_all(repo: &Repository, message: &str) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    
    // Add all changes
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;

    let oid = index.write_tree().map_err(|e| e.to_string())?;
    let signature = Signature::now("BookmarkSync", "sync@local").map_err(|e| e.to_string())?;
    let parent_commit = find_last_commit(repo)?;
    let tree = repo.find_tree(oid).map_err(|e| e.to_string())?;

    repo.commit(
        Some("HEAD"), // update HEAD
        &signature,   // author
        &signature,   // committer
        message,
        &tree,
        &[&parent_commit],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

fn find_last_commit(repo: &Repository) -> Result<git2::Commit<'_>, String> {
    let obj = repo.head().map_err(|e| e.to_string())?.resolve().map_err(|e| e.to_string())?.peel(git2::ObjectType::Commit).map_err(|e| e.to_string())?;
    obj.into_commit().map_err(|_| "Couldn't find commit".to_string())
}
