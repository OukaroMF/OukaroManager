use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::defs::config_path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    app: App,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct App {
    pub system_app: HashSet<String>,
    pub priv_app: HashSet<String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            app: App {
                system_app: HashSet::new(),
                priv_app: HashSet::new(),
            },
        }
    }

    /// load config
    pub fn load_config(&mut self) -> Result<()> {
        let config_path = config_path();
        let config = Path::new(&config_path);
        if !config.exists() {
            if let Some(parent) = config.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create config directory {}", parent.display()))?;
            }

            let toml = toml::to_string(&self).context("serialize default config")?;
            write_atomically(config, toml.as_bytes())?;
        }
        let buf = fs::read_to_string(config)
            .with_context(|| format!("read config {}", config.display()))?;
        let toml: Self = toml::from_str(buf.as_str())
            .with_context(|| format!("parse config {}", config.display()))?;
        self.app = toml.app;
        Ok(())
    }

    /// gef app config in local config
    pub fn get(&self) -> App {
        self.app.clone()
    }
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut temp = NamedTempFile::new_in(&parent)
        .with_context(|| format!("create temp file in {}", parent.display()))?;
    temp.write_all(contents)
        .with_context(|| format!("write temp config {}", path.display()))?;
    temp.flush()
        .with_context(|| format!("flush temp config {}", path.display()))?;
    temp.as_file()
        .sync_all()
        .with_context(|| format!("sync temp config {}", path.display()))?;

    if path.exists() {
        #[cfg(windows)]
        {
            fs::remove_file(path).with_context(|| format!("replace config {}", path.display()))?;
        }
    }

    temp.persist(path)
        .map_err(|err| err.error)
        .with_context(|| format!("persist config {}", path.display()))?;
    sync_directory(&parent).with_context(|| format!("sync config directory {}", parent.display()))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    let directory = fs::File::open(path)
        .with_context(|| format!("open config directory {}", path.display()))?;
    directory
        .sync_all()
        .with_context(|| format!("sync config directory {}", path.display()))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}
