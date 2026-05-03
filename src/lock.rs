use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use fs2::FileExt;

pub struct StoreLock {
    file: File,
}

impl StoreLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create lock directory {}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("open lock file {}", path.display()))?;

        file.try_lock_exclusive().map_err(|err| {
            anyhow!(
                "store is already locked at {} ({err}). Stop the other whatshell process or use another --store",
                path.display()
            )
        })?;

        file.set_len(0)?;
        writeln!(file, "pid={}", std::process::id())?;
        writeln!(file, "started_at={}", chrono::Utc::now().to_rfc3339())?;

        Ok(Self { file })
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
