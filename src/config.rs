use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub store_dir: PathBuf,
    pub json: bool,
    pub timeout_secs: u64,
    pub read_only: bool,
}

impl AppConfig {
    pub fn new(
        store_dir: Option<PathBuf>,
        json: bool,
        timeout_secs: u64,
        read_only: bool,
    ) -> Result<Self> {
        let store_dir = match store_dir {
            Some(path) => path,
            None => default_store_dir()?,
        };
        Ok(Self {
            store_dir,
            json,
            timeout_secs,
            read_only,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.store_dir)
            .with_context(|| format!("create store directory {}", self.store_dir.display()))?;
        fs::create_dir_all(self.media_dir())
            .with_context(|| format!("create media directory {}", self.media_dir().display()))?;
        Ok(())
    }

    pub fn session_db(&self) -> PathBuf {
        self.store_dir.join("session.db")
    }

    pub fn index_db(&self) -> PathBuf {
        self.store_dir.join("index.db")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.store_dir.join("LOCK")
    }

    pub fn media_dir(&self) -> PathBuf {
        self.store_dir.join("media")
    }
}

fn default_store_dir() -> Result<PathBuf> {
    if let Some(project_dirs) = ProjectDirs::from("dev", "wacli", "wacli") {
        return Ok(project_dirs.data_local_dir().to_path_buf());
    }

    let home = std::env::var_os("HOME").context("HOME is not set and no project data dir found")?;
    Ok(PathBuf::from(home).join(".wacli"))
}
