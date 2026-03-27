use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::defs::config_path;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Config {
    pub app: App,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct App {
    pub system_app: BTreeSet<String>,
    pub priv_app: BTreeSet<String>,
}

impl Config {
    pub fn new() -> Result<Self> {
        Self::load_from_path(config_path())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let buf = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config {}", path.display()))?;
        let config: Self =
            toml::from_str(buf.as_str()).with_context(|| "Failed to parse config".to_string())?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to_path(config_path())
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory {}", parent.display())
            })?;
        }

        let toml = toml::to_string(self).context("Failed to serialize config")?;
        write_atomically(path, toml.as_bytes())
    }
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut temp = NamedTempFile::new_in(&parent)
        .with_context(|| format!("Failed to create temp file in {}", parent.display()))?;
    temp.write_all(contents)
        .with_context(|| format!("Failed to write temp config for {}", path.display()))?;
    temp.flush()
        .with_context(|| format!("Failed to flush temp config for {}", path.display()))?;

    if path.exists() {
        #[cfg(windows)]
        {
            fs::remove_file(path)
                .with_context(|| format!("Failed to replace config {}", path.display()))?;
        }
    }

    temp.persist(path)
        .map_err(|err| err.error)
        .with_context(|| format!("Failed to persist config {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{App, Config};
    use std::collections::BTreeSet;

    use tempfile::tempdir;

    #[test]
    fn missing_config_loads_as_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = Config::load_from_path(&path).unwrap();

        assert_eq!(config, Config::default());
    }

    #[test]
    fn save_and_reload_round_trips_sorted_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = Config {
            app: App {
                system_app: BTreeSet::from([
                    "com.example.beta".to_string(),
                    "com.example.alpha".to_string(),
                ]),
                priv_app: BTreeSet::from(["com.example.gamma".to_string()]),
            },
        };

        config.save_to_path(&path).unwrap();
        let reloaded = Config::load_from_path(&path).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();

        assert_eq!(reloaded, config);
        assert!(contents.contains("system_app = [\"com.example.alpha\", \"com.example.beta\"]"));
    }
}
