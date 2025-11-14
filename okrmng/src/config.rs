use std::{collections::HashSet, fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::defs::CONFIG_PATH;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub app: App,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct App {
    pub system_app: HashSet<String>,
    pub priv_app: HashSet<String>,
}

impl Config {
    pub fn new() -> Result<Self> {
        let config = Path::new(CONFIG_PATH);
        if !config.exists() {
            panic!("config file is no exists!!");
        }
        let buf = fs::read_to_string(config)?;
        let toml: Self = toml::from_str(buf.as_str())?;
        Ok(Self { app: toml.app })
    }
}
