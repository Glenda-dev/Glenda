use serde::Deserialize;
use std::fs;
use std::path::Path;
#[derive(Debug, Deserialize)]
pub struct Service {
    pub name: String,
    pub path: String,
    pub build_cmd_debug: Option<String>,
    pub build_cmd_release: Option<String>,
    pub output: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Library {
    pub name: String,
    pub path: String,
    pub build_cmd_debug: Option<String>,
    pub build_cmd_release: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub features: std::collections::HashMap<String, Vec<String>>,
}

impl Config {
    pub fn from_path<P: AsRef<Path>>(p: P) -> anyhow::Result<Self> {
        let s = fs::read_to_string(p)?;
        let cfg: Config = toml::from_str(&s)?;
        Ok(cfg)
    }
}
