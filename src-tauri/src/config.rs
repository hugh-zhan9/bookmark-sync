use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataSourceKind {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub db: String,
    pub user: String,
    pub password: String,
    pub sslmode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub data_source: DataSourceKind,
    pub postgres: PostgresConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_source: DataSourceKind::Sqlite,
            postgres: PostgresConfig {
                host: "127.0.0.1".into(),
                port: 5432,
                db: "bookmark_sync".into(),
                user: "bookmark".into(),
                password: "".into(),
                sslmode: "prefer".into(),
            },
        }
    }
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load_or_init(dir: &Path) -> Result<AppConfig, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = config_path(dir);
    if !path.exists() {
        let cfg = AppConfig::default();
        save(dir, &cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let cfg: AppConfig = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(cfg)
}

pub fn save(dir: &Path, cfg: &AppConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = config_path(dir);
    let raw = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_or_init_should_create_default_config() {
        let dir = tempdir().expect("tmp dir");
        let cfg = load_or_init(dir.path()).expect("load_or_init");
        assert_eq!(cfg.data_source, DataSourceKind::Sqlite);
        let on_disk = load_or_init(dir.path()).expect("load_or_init again");
        assert_eq!(on_disk.data_source, DataSourceKind::Sqlite);
    }
}
