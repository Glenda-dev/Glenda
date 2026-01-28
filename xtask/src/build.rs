use crate::config::{Config, Service};
use crate::util::run;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build(cfg: &Config) -> anyhow::Result<()> {
    // Build libraries
    build_libraries(&cfg)?;
    // Process workspace services and generate initrd for kernel embedding
    build_initrd(&cfg)?;
    // Build the kernel
    build_kernel(&cfg)?;
    Ok(())
}

pub fn build_kernel(cfg: &Config) -> anyhow::Result<()> {
    let features = cfg.features.get("kernel").map(|arr| arr.join(",")).unwrap_or_default();
    let mut cmd = Command::new("cargo");
    cmd.current_dir("kernel");
    cmd.arg("build").arg("--target").arg(cfg.system.arch.target_triple());

    // Inject kernel linker script with absolute path
    let cwd = std::env::current_dir()?;
    let linker_script = cwd.join("kernel/src/hal").join(cfg.system.arch.as_str()).join("linker.ld");
    let rustflags = format!("-C link-arg=-T{} -C link-arg=--gc-sections", linker_script.display());
    cmd.env("RUSTFLAGS", rustflags);
    cmd.arg("--profile").arg(&cfg.system.profile);
    if !features.is_empty() {
        cmd.arg("--features").arg(features);
    }
    run(&mut cmd)?;

    // Copy binary to root target
    let profile = &cfg.system.profile;
    let src =
        Path::new("target").join(cfg.system.arch.target_triple()).join(profile).join("kernel");
    let dst = Path::new("target/kernel");
    fs::create_dir_all("target")?;
    fs::copy(src, dst)?;
    Ok(())
}

/// Run cargo build for a component
fn run_cargo_build(cfg: &Config, path: &Path, features: &str) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(path);
    cmd.arg("build");
    cmd.arg("--target").arg(cfg.system.arch.target_triple());
    cmd.arg("--profile").arg(&cfg.system.profile);
    if !features.is_empty() {
        cmd.arg("--features").arg(features);
    }
    run(&mut cmd)
}

pub fn build_libraries(cfg: &Config) -> anyhow::Result<()> {
    for c in cfg.libraries.iter() {
        let features = cfg.features.get(&c.name).map(|arr| arr.join(",")).unwrap_or_default();
        eprintln!("[ INFO ] Building Library {} with: {}", c.name, c.build);

        if c.build == "cargo" {
            run_cargo_build(cfg, Path::new(&c.path), &features)?;
        } else {
            anyhow::bail!("Unknown build method '{}' for library '{}'", c.build, c.name);
        }
    }

    Ok(())
}

/// Build a service with Cargo and return the path to the installed artifact
fn build_cargo_service(cfg: &Config, service: &Service) -> anyhow::Result<PathBuf> {
    let features = cfg.features.get(&service.name).map(|arr| arr.join(",")).unwrap_or_default();
    eprintln!("[ INFO ] Building Service {} with: cargo", service.name);

    // 1. Run Cargo Build
    run_cargo_build(cfg, Path::new(&service.path), &features)?;

    // 2. Identify Source Artifact
    // Assumption: Binary name matches service name
    // Artifact location: workspace_target_dir/target_triple/profile/name
    // Note: We assume we are running from workspace root
    let target_triple = cfg.system.arch.target_triple();
    let profile = &cfg.system.profile;
    let src_path = Path::new("target").join(target_triple).join(profile).join(&service.name);

    if !src_path.exists() {
        anyhow::bail!("Cargo build artifact not found at: {}", src_path.display());
    }

    // 3. Move/Copy to Output
    let dst_path = Path::new(&service.path).join(&service.output);
    if let Some(parent) = dst_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&src_path, &dst_path)?;

    Ok(dst_path)
}

const ENTRY_SIZE: usize = 48; // as in design

pub fn build_initrd(cfg: &Config) -> anyhow::Result<()> {
    // Ensure target dir
    fs::create_dir_all("target")?;

    // collect binaries
    let mut entries: Vec<(u8, String, Vec<u8>)> = Vec::new();

    // Helper to process a service and get its data
    let process_service = |c: &Service| -> anyhow::Result<(String, Vec<u8>)> {
        let artifact_path = match c.build.as_str() {
            "cargo" => build_cargo_service(cfg, c)?,
            "manifest" => Path::new(&c.path).join(&c.output),
            _ => anyhow::bail!("Unknown build method '{}' for service '{}'", c.build, c.name),
        };

        if !artifact_path.exists() {
            anyhow::bail!("Service artifact not found at: {}", artifact_path.display());
        }

        let data = fs::read(&artifact_path)?;
        Ok((c.name.clone(), data))
    };

    // 1. Find and process Root Task first
    if let Some(root_task_cfg) = cfg.services.iter().find(|c| c.kind == "root_task") {
        let (name, data) = process_service(root_task_cfg)?;
        entries.push((0, name, data));
    } else {
        eprintln!("[ WARN ] No root_task defined in config.toml");
    }

    // 2. Process other services
    for c in cfg.services.iter() {
        if c.kind == "root_task" {
            continue; // Already processed
        }

        let (name, data) = process_service(c)?;

        // kind mapping
        let t: u8 = match c.kind.as_str() {
            "driver" => 1,
            "server" => 2,
            "test" => 3,
            "file" => 4,
            _ => 4,
        };
        entries.push((t, name, data));
    }

    // build modules.bin in target/modules.bin
    let modules_path = Path::new("target").join("modules.bin");
    let mut file = File::create(&modules_path)?;

    // header: magic + count + total_size + padding
    const MAGIC: u32 = 0x99999999;
    let count = entries.len() as u32;

    // compute sizes to populate header
    let header_size = 16; // 4 + 4 + 4 + 4(padding)
    let entries_size = (entries.len() * ENTRY_SIZE) as u32;

    // compute offsets and total size with alignment
    let mut current_data_offset = header_size + entries_size;
    let mut aligned_entries = Vec::new();

    for (t, name, data) in entries {
        let start = (current_data_offset + 7) & !7; // 8-byte alignment
        let size = data.len() as u32;
        aligned_entries.push((t, name, data, start));
        current_data_offset = start + size;
    }
    let total_size = current_data_offset;

    file.write_all(&MAGIC.to_le_bytes())?;
    file.write_all(&count.to_le_bytes())?;
    file.write_all(&total_size.to_le_bytes())?;
    file.write_all(&[0u8; 4])?; // Padding to 16 bytes

    // write metadata entries
    for (t, name, _data, start) in aligned_entries.iter() {
        // type
        file.write_all(&[*t])?;
        // offset
        file.write_all(&start.to_le_bytes())?;
        // size
        let size = _data.len() as u32;
        file.write_all(&size.to_le_bytes())?;
        // name (32 bytes, null padded)
        let mut name_buf = [0u8; 32];
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        name_buf[..len].copy_from_slice(&bytes[..len]);
        file.write_all(&name_buf)?;
        // padding 7 bytes
        file.write_all(&[0u8; 7])?;
    }

    // write data with padding
    let mut current_pos = header_size + entries_size;
    for (_t, _name, data, start) in aligned_entries.into_iter() {
        // Write padding bytes
        if start > current_pos {
            file.write_all(&vec![0u8; (start - current_pos) as usize])?;
        }
        file.write_all(&data)?;
        current_pos = start + data.len() as u32;
    }

    eprintln!("[ INFO ] Wrote initrd ({} KB) to {}", total_size / 1024, modules_path.display());

    Ok(())
}

pub fn clean() -> anyhow::Result<()> {
    // Remove target dir
    let target_path = Path::new("target");
    if target_path.exists() {
        fs::remove_dir_all(target_path)?;
    }

    eprintln!("[ INFO ] Cleaned build artifacts");

    Ok(())
}
