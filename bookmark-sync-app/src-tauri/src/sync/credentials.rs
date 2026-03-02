use keyring::Entry;

// The service name inside macOS Keychain or Windows Credential Manager
const SERVICE_NAME: &str = "bookmark-sync-app";
// The key under which we store the Personal Access Token (PAT)
const TOKEN_KEY: &str = "github-pat";
// The key under which we store the Repo URL e.g., git@github.com:user/bookmarks.git
const REPO_KEY: &str = "github-repo-url";

pub fn save_credentials(repo_url: &str, token: &str) -> Result<(), String> {
    let repo_entry = Entry::new(SERVICE_NAME, REPO_KEY).map_err(|e| e.to_string())?;
    repo_entry.set_password(repo_url).map_err(|e| e.to_string())?;

    let token_entry = Entry::new(SERVICE_NAME, TOKEN_KEY).map_err(|e| e.to_string())?;
    token_entry.set_password(token).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn get_credentials() -> Result<(String, String), String> {
    let repo_entry = Entry::new(SERVICE_NAME, REPO_KEY).map_err(|e| e.to_string())?;
    let repo_url = repo_entry.get_password().map_err(|e| e.to_string())?;

    let token_entry = Entry::new(SERVICE_NAME, TOKEN_KEY).map_err(|e| e.to_string())?;
    let token = token_entry.get_password().map_err(|e| e.to_string())?;

    Ok((repo_url, token))
}

pub fn clear_credentials() -> Result<(), String> {
    let repo_entry = Entry::new(SERVICE_NAME, REPO_KEY).map_err(|e| e.to_string())?;
    let _ = repo_entry.delete_credential();

    let token_entry = Entry::new(SERVICE_NAME, TOKEN_KEY).map_err(|e| e.to_string())?;
    let _ = token_entry.delete_credential();

    Ok(())
}
