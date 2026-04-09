use crate::arch::{Arch, Bootloader};
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
    pub output: String,
}

#[derive(Debug, Deserialize)]
pub struct SystemConfig {
    #[serde(default)]
    pub arch: Arch,
    #[serde(default)]
    pub bootloader: Bootloader,
    #[serde(default = "release")]
    pub profile: String,
}

fn default_cpus() -> u32 {
    1
}

fn default_mem() -> String {
    "1G".into()
}

fn default_display() -> String {
    "gtk".into()
}

#[derive(Debug, Deserialize)]
pub struct QemuConfig {
    #[serde(default = "default_cpus")]
    pub cpus: u32,
    #[serde(default = "default_mem")]
    pub mem: String,
    #[serde(default = "default_display")]
    pub display: String,
    #[serde(default)]
    pub disk: Option<String>,
    #[serde(default)]
    pub drive: Option<String>,
    #[serde(default)]
    pub net: bool,
    #[serde(default)]
    pub net_type: Option<String>,
    #[serde(default)]
    pub mac: Option<String>,
    #[serde(default)]
    pub hostfwd: Option<Vec<String>>,
    #[serde(default)]
    pub bios: Option<String>,
}

impl Default for QemuConfig {
    fn default() -> Self {
        Self {
            cpus: default_cpus(),
            mem: default_mem(),
            display: default_display(),
            disk: None,
            drive: None,
            net: false,
            net_type: None,
            mac: None,
            hostfwd: None,
            bios: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct HostedConfig {
    pub runtime: String,
    pub socket: String,
    pub apps: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub files: Vec<File>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub features: HashMap<String, Vec<String>>,
    pub system: SystemConfig,
    #[serde(default)]
    pub qemu: QemuConfig,
    #[serde(default)]
    pub hosted: HostedConfig,
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
        Self { arch: Arch::default(), bootloader: Bootloader::default(), profile: release() }
    }
}
