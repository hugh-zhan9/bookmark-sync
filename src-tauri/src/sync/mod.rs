pub mod credentials;

use std::path::PathBuf;
use std::fs;
use std::process::Command;
use git2::{Repository, RemoteCallbacks, FetchOptions, PushOptions, Cred, Signature, build::CheckoutBuilder};

pub fn init_or_open_repo(app_data_dir: &PathBuf, repo_url: &str, token: &str) -> Result<Repository, String> {
    let repo_path = app_data_dir.join("sync-repo");
    
    if repo_path.exists() {
        Repository::open(&repo_path).map_err(|e| format!("Failed to open existing repo: {}", e))
    } else {
        // Clone with token in Auth
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, _username_from_url, _allowed_types| {
            Cred::userpass_plaintext("git", token)
        });

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        
        builder.clone(repo_url, &repo_path).map_err(|e| format!("Clone failed: {}", e))
    }
}

pub fn commit_all(repo: &Repository, message: &str) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("bookmark-sync-app", "sync@local"))
        .map_err(|e| e.to_string())?;
    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    
    let mut parents = Vec::new();
    if let Some(ref pc) = parent_commit {
        parents.push(pc);
    }
    
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn pull_main(repo: &Repository, token: &str) -> Result<(), String> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, _username_from_url, _allowed_types| {
        Cred::userpass_plaintext("git", token)
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
    remote
        .fetch(&["refs/heads/main:refs/remotes/origin/main"], Some(&mut fetch_options), None)
        .map_err(|e| e.to_string())?;

    let reference_name = "refs/heads/main";
    let target_ref = repo
        .find_reference("refs/remotes/origin/main")
        .map_err(|e| e.to_string())?;
    let target_oid = target_ref.target().ok_or_else(|| "origin/main has no target".to_string())?;

    match repo.find_reference(reference_name) {
        Ok(mut local_ref) => {
            local_ref.set_target(target_oid, "fast-forward").map_err(|e| e.to_string())?;
        }
        Err(_) => {
            repo.reference(reference_name, target_oid, true, "create main")
                .map_err(|e| e.to_string())?;
        }
    }

    repo.set_head(reference_name).map_err(|e| e.to_string())?;
    let mut checkout = CheckoutBuilder::new();
    checkout.force();
    repo.checkout_head(Some(&mut checkout)).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn push_main(repo: &Repository, token: &str) -> Result<(), String> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, _username_from_url, _allowed_types| {
        Cred::userpass_plaintext("git", token)
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
    remote
        .push(&["refs/heads/main:refs/heads/main"], Some(&mut push_options))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn sync_db_snapshot(app_data_dir: &PathBuf, repo_url: &str, token: &str) -> Result<(), String> {
    let repo = init_or_open_repo(app_data_dir, repo_url, token)?;
    let repo_path = app_data_dir.join("sync-repo");
    if repo_path.exists() {
        let _ = pull_main(&repo, token);
    }

    let db_path = app_data_dir.join("bookmarks.db");
    if !db_path.exists() {
        return Err("bookmarks.db not found".to_string());
    }

    let target = repo_path.join("bookmarks.db");
    fs::copy(&db_path, &target).map_err(|e| e.to_string())?;
    commit_all(&repo, &format!("sync bookmarks {}", chrono::Utc::now().to_rfc3339()))?;
    push_main(&repo, token)?;
    Ok(())
}

pub fn is_git_repo_dir(repo_dir: &str) -> bool {
    Repository::open(repo_dir).is_ok()
}

pub fn current_branch(repo_dir: &str) -> Result<String, String> {
    let repo = Repository::open(repo_dir).map_err(|e| e.to_string())?;
    let head = repo.head().map_err(|e| e.to_string())?;
    let branch = head.shorthand().ok_or_else(|| "无法获取当前分支".to_string())?;
    Ok(branch.to_string())
}

pub fn git_pull_current_branch(repo_dir: &str) -> Result<(), String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("pull")
        .arg("--rebase")
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("git pull --rebase failed".to_string());
    }
    Ok(())
}

pub fn git_add_commit_push_current_branch(repo_dir: &str, rel_path: &str, message: &str) -> Result<(), String> {
    let add = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("add")
        .arg(rel_path)
        .status()
        .map_err(|e| e.to_string())?;
    if !add.success() {
        return Err("git add failed".to_string());
    }

    let has_changes = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .status()
        .map_err(|e| e.to_string())?;
    if has_changes.success() {
        return Ok(());
    }

    let commit = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("commit")
        .arg("-m")
        .arg(message)
        .status()
        .map_err(|e| e.to_string())?;
    if !commit.success() {
        return Err("git commit failed".to_string());
    }

    let push = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("push")
        .status()
        .map_err(|e| e.to_string())?;
    if !push.success() {
        return Err("git push failed".to_string());
    }
    Ok(())
}

pub fn ensure_events_dir(repo_dir: &str) -> Result<PathBuf, String> {
    let events_dir = PathBuf::from(repo_dir).join("events");
    fs::create_dir_all(&events_dir).map_err(|e| e.to_string())?;
    Ok(events_dir)
}
