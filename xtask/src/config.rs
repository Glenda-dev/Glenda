use crate::arch::Arch;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Service {
    pub name: String,
    pub path: String,
    pub build: String,
    pub output: String,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub struct Library {
    pub name: String,
    pub path: String,
    pub build: String,
}

#[derive(Debug, Deserialize)]
pub struct SystemConfig {
    #[serde(default)]
    pub arch: Arch,
    #[serde(default = "release")]
    pub profile: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub features: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub system: SystemConfig,
}

impl Config {
    pub fn from_path<P: AsRef<Path>>(p: P) -> anyhow::Result<Self> {
        let s = fs::read_to_string(p)?;
        let cfg: Config = toml::from_str(&s)?;
        Ok(cfg)
    }
}

fn release() -> String {
    "release".into()
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self { arch: Arch::default(), profile: release() }
    }
}
