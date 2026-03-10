use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::config::{AppConfig, DataSourceKind};
use crate::db;
use crate::db::postgres;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_postgres::postgres::tls::NoTls;

pub struct DbRouter {
    kind: DataSourceKind,
    sqlite: Option<Arc<Mutex<Connection>>>,
    pg: Option<Pool<PostgresConnectionManager<NoTls>>>,
    app_data_dir: PathBuf,
}

impl DbRouter {
    pub fn init(cfg: &AppConfig, app_data_dir: PathBuf) -> Result<Self, String> {
        let mut router = Self {
            kind: cfg.data_source,
            sqlite: None,
            pg: None,
            app_data_dir,
        };
        router.reinit(cfg)?;
        Ok(router)
    }

    pub fn kind(&self) -> DataSourceKind {
        self.kind
    }

    pub fn app_data_dir(&self) -> &PathBuf {
        &self.app_data_dir
    }

    pub fn reinit(&mut self, cfg: &AppConfig) -> Result<(), String> {
        self.kind = cfg.data_source;
        match cfg.data_source {
            DataSourceKind::Sqlite => {
                let conn = db::init_db(self.app_data_dir.clone()).map_err(|e| e.to_string())?;
                self.sqlite = Some(Arc::new(Mutex::new(conn)));
                self.pg = None;
            }
            DataSourceKind::Postgres => {
                let pool = postgres::init_db(&cfg.postgres)?;
                self.pg = Some(pool);
                self.sqlite = None;
            }
        }
        Ok(())
    }

    pub fn sqlite_conn(&self) -> Result<Arc<Mutex<Connection>>, String> {
        self.sqlite
            .as_ref()
            .ok_or_else(|| "sqlite unavailable".to_string())
            .map(Arc::clone)
    }

    pub fn pg_pool(&self) -> Result<&Pool<PostgresConnectionManager<NoTls>>, String> {
        self.pg.as_ref().ok_or_else(|| "postgres unavailable".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn router_should_init_sqlite_by_default() {
        let dir = tempdir().expect("tmp dir");
        let cfg = AppConfig::default();
        let router = DbRouter::init(&cfg, dir.path().to_path_buf()).expect("init");
        assert_eq!(router.kind(), DataSourceKind::Sqlite);
    }
}
