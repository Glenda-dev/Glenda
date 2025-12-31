use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use which::which;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "Glenda Build System")]
struct Xtask {
    #[arg(long, global = true)]
    release: bool,

    #[arg(long = "features", value_delimiter = ',', num_args(0..), global = true)]
    features: Vec<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Build the kernel
    Build,
    /// Build then boot the kernel in QEMU
    Run {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "128M")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,
    },
    /// Run kernel tests
    Test {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "128M")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,
    },
    /// Start QEMU paused and wait for GDB
    Gdb {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "128M")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,

        /// Run tests instead of normal kernel
        #[arg(long, default_value_t = false)]
        test: bool,
    },
    /// Disassemble the kernel ELF
    Objdump,
    /// Show section sizes
    Size,
    /// Generate disk.img
    Mkfs,
}

fn main() -> anyhow::Result<()> {
    let xtask = Xtask::parse();
    let mode = if xtask.release { "release" } else { "debug" };

    match xtask.cmd {
        Cmd::Build => build(mode, &xtask.features)?,
        Cmd::Run { cpus, mem, display } => {
            build(mode, &xtask.features)?;
            mkfs()?;
            qemu_run(mode, cpus, &mem, &display)?;
        }
        Cmd::Gdb { cpus, mem, display, test } => {
            let mut feats = xtask.features.clone();
            if test == true {
                if !feats.iter().any(|f| f == "tests") {
                    feats.push(String::from("tests"));
                }
            }
            build(mode, &feats)?;
            mkfs()?;
            qemu_gdb(mode, cpus, &mem, &display)?;
        }
        Cmd::Test { cpus, mem, display } => {
            let mut feats = xtask.features.clone();
            if !feats.iter().any(|f| f == "tests") {
                feats.push(String::from("tests"));
            }
            build(mode, &feats)?;
            mkfs()?;
            qemu_run(mode, cpus, &mem, &display)?;
        }
        Cmd::Objdump => objdump(mode)?,
        Cmd::Size => size(mode)?,
        Cmd::Mkfs => mkfs()?,
    }
    Ok(())
}

fn mkfs() -> anyhow::Result<()> {
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    // Parameters
    const BLOCK_SIZE: usize = 4096;
    const N_INODES: usize = 200;
    const N_DATA_BLOCKS: usize = 1000;
    const MAGIC: u32 = 0x10203040;

    // Sizes
    let sb_size = 1;
    let inode_bitmap_size = 1;

    // Inode size 64 bytes
    const IPB: usize = BLOCK_SIZE / 64;
    let inode_blocks = (N_INODES + IPB - 1) / IPB;

    let data_bitmap_size = 1;

    let total_blocks =
        sb_size + inode_bitmap_size + inode_blocks + data_bitmap_size + N_DATA_BLOCKS;

    let inode_region_start = sb_size + inode_bitmap_size;
    let data_bitmap_start = inode_region_start + inode_blocks;

    println!(
        "[ INFO ] Generating disk.img (Size: {} blocks / {} bytes)",
        total_blocks,
        total_blocks * BLOCK_SIZE
    );
    println!(
        "[ INFO ] Layout: SB:0, IBMap:1, IRegions:{}-{}, DBMap:{}, Data:{}...",
        inode_region_start,
        inode_region_start + inode_blocks - 1,
        data_bitmap_start,
        data_bitmap_start + 1
    );

    let mut file = File::create("disk.img")?;

    file.set_len((total_blocks * BLOCK_SIZE) as u64)?;

    let mut sb_buf = [0u8; BLOCK_SIZE];
    let magic_bytes = MAGIC.to_le_bytes();
    let size_bytes = (total_blocks as u32).to_le_bytes();
    let nblocks_bytes = (N_DATA_BLOCKS as u32).to_le_bytes();
    let ninodes_bytes = (N_INODES as u32).to_le_bytes();
    let inode_start_bytes = (inode_region_start as u32).to_le_bytes();
    let bmap_start_bytes = (data_bitmap_start as u32).to_le_bytes();

    sb_buf[0..4].copy_from_slice(&magic_bytes);
    sb_buf[4..8].copy_from_slice(&size_bytes);
    sb_buf[8..12].copy_from_slice(&nblocks_bytes);
    sb_buf[12..16].copy_from_slice(&ninodes_bytes);
    sb_buf[16..20].copy_from_slice(&inode_start_bytes);
    sb_buf[20..24].copy_from_slice(&bmap_start_bytes);

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&sb_buf)?;

    let rich = match std::env::var("GLENDA_RICH_MKFS") {
        std::result::Result::Ok(v) => {
            let v = v.to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        }
        std::result::Result::Err(_) => false,
    };
    if !rich {
        // Leave inode/data bitmaps and regions zeroed; kernel tests will create root/dentries.
        return Ok(());
    }

    let mut write_block = |blk: u64, data: &[u8]| -> anyhow::Result<()> {
        if data.len() != BLOCK_SIZE { return Err(anyhow::anyhow!("block size mismatch")); }
        file.seek(SeekFrom::Start(blk * BLOCK_SIZE as u64))?;
        file.write_all(data)?;
        Ok(())
    };

    let mut zero_block = || -> [u8; BLOCK_SIZE] { [0u8; BLOCK_SIZE] };

    // Derived constants for FS content
    const ROOT_INODE: u32 = 0;
    const INODE_INDEX_3: usize = 13; // 10 direct + 2 single indirect + 1 double indirect
    const MAXLEN_FILENAME: usize = 60; // Make dentry 64 bytes total
    const INODE_SIZE: usize = 64; // On-disk inode size
    const DENTRY_SIZE: usize = 64; // On-disk dentry size
    let _ipb = BLOCK_SIZE / INODE_SIZE; // inodes per block
    let data_start = data_bitmap_start + 1; // absolute block of first data block

    // Inode bitmap: mark 0,1,2 as used
    let mut ibmap = zero_block();
    for inum in 0..3u32 {
        let byte_idx = (inum / 8) as usize;
        let bit = (inum % 8) as u8;
        ibmap[byte_idx] |= 1u8 << bit;
    }
    write_block(1, &ibmap)?; // ibmap is fixed at block 1

    // Data bitmap: allocate 3 blocks (root dir + 2 files)
    let mut dbmap = zero_block();
    for bit_idx in 0..3u32 {
        let byte_idx = (bit_idx / 8) as usize;
        let bit = (bit_idx % 8) as u8;
        dbmap[byte_idx] |= 1u8 << bit;
    }
    write_block(data_bitmap_start as u64, &dbmap)?;

    // Build inodes (inum 0=root dir, 1=ABCD.txt, 2=abcd.txt)
    let mut inode_block0 = zero_block();
    let mut put_inode = |buf: &mut [u8], slot: usize,
                         typ: u16, major: u16, minor: u16, nlink: u16,
                         size: u32, idx0: u32| {
        let base = slot * INODE_SIZE;
        // Layout: u16 type, u16 major, u16 minor, u16 nlink, u32 size, u32 index[13]
        buf[base + 0..base + 2].copy_from_slice(&typ.to_le_bytes());
        buf[base + 2..base + 4].copy_from_slice(&major.to_le_bytes());
        buf[base + 4..base + 6].copy_from_slice(&minor.to_le_bytes());
        buf[base + 6..base + 8].copy_from_slice(&nlink.to_le_bytes());
        buf[base + 8..base + 12].copy_from_slice(&size.to_le_bytes());
        // Clear index area first
        for i in 0..INODE_INDEX_3 {
            let off = base + 12 + i * 4;
            buf[off..off + 4].copy_from_slice(&0u32.to_le_bytes());
        }
        // index[0]
        buf[base + 12..base + 16].copy_from_slice(&idx0.to_le_bytes());
    };

    let root_dir_block = (data_start + 0) as u32;
    let upper_block = (data_start + 1) as u32;
    let lower_block = (data_start + 2) as u32;

    // type: 1=DIR, 2=DATA (kept consistent with kernel)
    put_inode(&mut inode_block0, 0, 1, 0, 0, 1, 4 * DENTRY_SIZE as u32, root_dir_block);
    put_inode(&mut inode_block0, 1, 2, 0, 0, 1, BLOCK_SIZE as u32, upper_block);
    put_inode(&mut inode_block0, 2, 2, 0, 0, 1, BLOCK_SIZE as u32, lower_block);
    write_block(inode_region_start as u64, &inode_block0)?;

    // Zero remaining inode blocks (optional)
    if inode_blocks > 1 {
        let zb = zero_block();
        for i in 1..inode_blocks {
            write_block((inode_region_start + i) as u64, &zb)?;
        }
    }

    // Root directory block
    let mut dir_block = zero_block();
    let mut put_dentry = |buf: &mut [u8], slot: usize, name: &str, inum: u32| {
        let base = slot * DENTRY_SIZE;
        let name_bytes = name.as_bytes();
        let copy_len = core::cmp::min(name_bytes.len(), MAXLEN_FILENAME);
        buf[base..base + copy_len].copy_from_slice(&name_bytes[..copy_len]);
        buf[base + MAXLEN_FILENAME..base + MAXLEN_FILENAME + 4].copy_from_slice(&inum.to_le_bytes());
    };
    put_dentry(&mut dir_block, 0, ".", ROOT_INODE);
    put_dentry(&mut dir_block, 1, "..", ROOT_INODE);
    put_dentry(&mut dir_block, 2, "ABCD.txt", 1);
    put_dentry(&mut dir_block, 3, "abcd.txt", 2);
    write_block(root_dir_block as u64, &dir_block)?;

    // File data blocks
    let mut upper = zero_block();
    let mut lower = zero_block();
    for i in 0..BLOCK_SIZE {
        upper[i] = b'A' + (i % 26) as u8;
        lower[i] = b'a' + (i % 26) as u8;
    }
    write_block(upper_block as u64, &upper)?;
    write_block(lower_block as u64, &lower)?;

    Ok(())
}

fn elf_path(mode: &str) -> PathBuf {
    Path::new("target").join("riscv64gc-unknown-none-elf").join(mode).join("kernel")
}

fn build(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    build_service(mode, features)?;
    link_service(mode, features)?;
    build_kernel(mode, features)?;
    Ok(())
}

fn build_kernel(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("-p").arg("kernel").arg("--target").arg("riscv64gc-unknown-none-elf");
    if mode == "release" {
        cmd.arg("--release");
    }
    if !features.is_empty() {
        let joined = features.join(",");
        cmd.arg("--features").arg(joined);
    }
    run(&mut cmd)
}

fn build_service(_mode: &str, _features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("make");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    std::fs::create_dir_all(format!("{}/../target/service/hello", manifest_dir)).unwrap();
    cmd.current_dir(format!("{}/../service/hello", manifest_dir));
    cmd.arg("CROSS_COMPILE=riscv64-unknown-elf-");
    run(&mut cmd)
}

// Mode and Features are expected to be unused now
fn link_service(_mode: &str, _features: &Vec<String>) -> anyhow::Result<()> {
    let service_bin =
        std::path::Path::new("target").join("service").join("hello").join("hello.bin");
    let service_bin_str = "service/hello/hello.bin";
    let out_file = std::path::Path::new("target").join("proc_payload.rs");
    if service_bin.exists() {
        let content = format!(
            "pub const PROC_PAYLOAD: &[u8] = include_bytes!(\"{}\");\npub const HAS_PROC_PAYLOAD: bool = true;\n",
            service_bin_str
        );
        std::fs::write(&out_file, content).unwrap();
    } else {
        println!(
            "[ WARN ] Service binary not found: {}, generating empty payload",
            service_bin.display(),
        );
        let content = String::from(
            "pub const PROC_PAYLOAD: &[u8] = &[];\npub const HAS_PROC_PAYLOAD: bool = false;\n",
        );
        std::fs::write(&out_file, content).unwrap();
    }
    Ok(())
}

fn qemu_cmd() -> anyhow::Result<String> {
    let qemu = which("qemu-system-riscv64")
        .map_err(|_| anyhow::anyhow!("[ ERROR ] qemu-system-riscv64 not found in PATH"))?;
    Ok(qemu.to_string_lossy().into_owned())
}

fn qemu_run(mode: &str, cpus: u32, mem: &str, display: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd()?;
    let mut cmd = Command::new(&qemu);
    cmd.arg("-machine").arg("virt");
    // CPUs
    if cpus > 1 {
        cmd.arg("-smp").arg(cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(mem);
    // Display handling: keep legacy -nographic behavior when requested
    if display == "nographic" {
        cmd.arg("-nographic");
    } else if display == "none" {
        cmd.arg("-display").arg("none");
    } else {
        // pass raw display backend name (e.g. gtk, sdl)
        cmd.arg("-display").arg(display);
    }
    cmd.arg("-drive").arg("file=disk.img,if=none,format=raw,id=x0");
    cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
    cmd.arg("-bios").arg("default").arg("-kernel").arg(elf.to_str().unwrap());
    run(&mut cmd)
}

fn qemu_gdb(mode: &str, cpus: u32, mem: &str, display: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd()?;
    let mut cmd = Command::new(&qemu);
    cmd.arg("-machine").arg("virt");
    // CPUs
    if cpus > 1 {
        cmd.arg("-smp").arg(cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(mem);
    // Display handling
    if display == "nographic" {
        cmd.arg("-nographic");
    } else if display == "none" {
        cmd.arg("-display").arg("none");
    } else {
        cmd.arg("-display").arg(display);
    }
    cmd.arg("-drive").arg("file=disk.img,if=none,format=raw,id=x0");
    cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
    cmd.arg("-bios").arg("default").arg("-S").arg("-s").arg("-kernel").arg(elf.to_str().unwrap());
    eprintln!("QEMU started. In another shell:");
    if which("gdb").is_ok() {
        eprintln!("  gdb -ex 'set architecture riscv:rv64' -ex 'target remote :1234' -ex 'symbol-file {}'", elf.display());
    } else {
        eprintln!("[ ERROR ] install gdb or riscv64elf-gdb first");
    }
    run(&mut cmd)
}

fn objdump(mode: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    let tool = which("riscv64-elf-objdump")
        .or_else(|_| which("llvm-objdump"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install objdump first"))?;
    let mut cmd = Command::new(tool);
    if cmd.get_program().to_string_lossy().contains("llvm-objdump") {
        cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    } else {
        cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    }
    run(&mut cmd)
}

fn size(mode: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    let tool = which("riscv64-elf-size")
        .or_else(|_| which("size"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install size first"))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-A", elf.to_str().unwrap()]);
    run(&mut cmd)
}

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Running: $ {:?}", cmd);
    let status =
        cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("[ ERROR ] command failed with status {}", status));
    }
    Ok(())
}

mod anyhow {
    pub use anyhow::*;
}
use anyhow::*;
