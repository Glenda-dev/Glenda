use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Service {
    pub name: String,
    pub path: String,
    pub build_cmd: Option<String>,
    pub output_bin: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(rename = "services")]
    pub services: Vec<Service>,
}

impl Config {
    pub fn from_path<P: AsRef<Path>>(p: P) -> anyhow::Result<Self> {
        let s = fs::read_to_string(p)?;
        let cfg: Config = toml::from_str(&s)?;
        Ok(cfg)
    }
}
