use crate::config::Config;
use crate::util::run;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub fn build(mode: &str, features: &Vec<String>, config_path: Option<&str>) -> anyhow::Result<()> {
    // Build libraries
    build_libraries(mode, features)?;
    // Process workspace services and generate modules blob for kernel embedding
    build_services(config_path)?;
    // Build the kernel
    build_kernel(mode, features)?;
    Ok(())
}

pub fn build_kernel(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir("kernel");
    cmd.arg("build").arg("--target").arg("riscv64gc-unknown-none-elf");

    // Inject kernel linker script with absolute path
    let cwd = std::env::current_dir()?;
    let linker_script = cwd.join("kernel/src/linker.ld");
    let rustflags = format!("-C link-arg=-T{} -C link-arg=--gc-sections", linker_script.display());
    cmd.env("RUSTFLAGS", rustflags);

    if mode == "release" {
        cmd.arg("--release");
    }
    if !features.is_empty() {
        let joined = features.join(",");
        cmd.arg("--features").arg(joined);
    }
    run(&mut cmd)?;

    // Copy binary to root target
    let profile = if mode == "release" { "release" } else { "debug" };
    let src = Path::new("target/riscv64gc-unknown-none-elf").join(profile).join("kernel");
    let dst = Path::new("target/kernel");
    fs::create_dir_all("target")?;
    fs::copy(src, dst)?;
    Ok(())
}

pub fn build_libraries(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir("lib/libglenda-rs");
    cmd.arg("build").arg("--target").arg("riscv64gc-unknown-none-elf");
    if mode == "release" {
        cmd.arg("--release");
    }
    if !features.is_empty() {
        let joined = features.join(",");
        cmd.arg("--features").arg(joined);
    }
    run(&mut cmd)
}

const ENTRY_SIZE: usize = 48; // as in design

pub fn build_services(config_path: Option<&str>) -> anyhow::Result<()> {
    let default_path = "config.toml";
    let cfg_path = Path::new(config_path.unwrap_or(default_path));
    if !cfg_path.exists() {
        eprintln!("[ WARN ] {} not found, skipping pack step", cfg_path.display());
        return Ok(());
    }
    let cfg = Config::from_path(cfg_path)?;

    // Ensure target dir
    fs::create_dir_all("target")?;

    // collect binaries
    let mut entries: Vec<(u8, String, Vec<u8>)> = Vec::new();

    // 1. Find and process Root Task first
    if let Some(root_task_cfg) =
        cfg.services.iter().find(|c| c.kind.as_deref() == Some("root_task"))
    {
        if let Some(cmd_str) = &root_task_cfg.build_cmd {
            eprintln!("[ INFO ] Building Root Task {} with: {}", root_task_cfg.name, cmd_str);
            let status = Command::new("sh")
                .arg("-c")
                .arg(cmd_str)
                .current_dir(&root_task_cfg.path)
                .status()?;
            if !status.success() {
                return Err(anyhow::anyhow!("build command failed for {}", root_task_cfg.name));
            }
        }
        let out_path = Path::new(&(root_task_cfg.path)).join(&root_task_cfg.output_bin);
        if !out_path.exists() {
            return Err(anyhow::anyhow!("output binary not found: {}", root_task_cfg.output_bin));
        }

        // Copy to root target
        let dst = Path::new("target").join(format!("{}.bin", root_task_cfg.name));
        fs::copy(&out_path, &dst)?;

        let data = fs::read(&dst)?;
        entries.push((0, root_task_cfg.name.clone(), data));
    } else {
        eprintln!("[ WARN ] No root_task defined in config.toml");
    }

    // 2. Process other services
    for c in cfg.services.iter() {
        if c.kind.as_deref() == Some("root_task") {
            continue; // Already processed
        }
        if let Some(cmd_str) = &c.build_cmd {
            eprintln!("[ INFO ] Building component {} with: {}", c.name, cmd_str);
            // run via shell so build_cmd can be arbitrary
            let status = Command::new("sh").arg("-c").arg(cmd_str).current_dir(&c.path).status()?;
            if !status.success() {
                return Err(anyhow::anyhow!("build command failed for {}", c.name));
            }
        }
        let out_path = Path::new(&(c.path)).join(&c.output_bin);
        if !out_path.exists() {
            return Err(anyhow::anyhow!("output binary not found: {}", c.output_bin));
        }

        // Copy to root target
        let dst = Path::new("target").join(format!("{}.bin", c.name));
        fs::copy(&out_path, &dst)?;

        let data = fs::read(&dst)?;
        // kind mapping
        let t: u8 = match c.kind.as_deref().unwrap_or("file") {
            "driver" => 1,
            "server" => 2,
            "test" => 3,
            "file" => 4,
            _ => 4,
        };
        entries.push((t, c.name.clone(), data));
    }

    // build modules.bin in target/modules.bin
    let modules_path = Path::new("target").join("modules.bin");
    let mut file = File::create(&modules_path)?;

    // header: magic + count + total_size
    const MAGIC: u32 = 0x99999999;
    let count = entries.len() as u32;

    // compute sizes to populate header
    let header_size = 4 + 4 + 4; // magic + count + total_size
    let entries_size = (entries.len() * ENTRY_SIZE) as u32;
    let data_size: u32 = entries.iter().map(|(_t, _n, d)| d.len() as u32).sum();
    let total_size = header_size as u32 + entries_size + data_size;

    file.write_all(&MAGIC.to_le_bytes())?;
    file.write_all(&count.to_le_bytes())?;
    file.write_all(&total_size.to_le_bytes())?;

    // compute offsets: header + entries
    let mut offset = header_size as u32 + entries_size;

    // write metadata entries
    for (t, name, data) in entries.iter() {
        // type
        file.write_all(&[*t])?;
        // offset
        file.write_all(&offset.to_le_bytes())?;
        // size
        let size = data.len() as u32;
        file.write_all(&size.to_le_bytes())?;
        // name (32 bytes, null padded)
        let mut name_buf = [0u8; 32];
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        name_buf[..len].copy_from_slice(&bytes[..len]);
        file.write_all(&name_buf)?;
        // padding 7 bytes
        file.write_all(&[0u8; 7])?;
        offset += size;
    }

    // write data
    for (_t, _name, data) in entries.into_iter() {
        file.write_all(&data)?;
    }

    eprintln!("[ INFO ] Wrote modules blob to {}", modules_path.display());

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
