//! Kernel build script (toolchain module scaffolding).
//!
//! This build script is intentionally minimal: the kernel targets are built by
//! `cargo xtask build` and linked from the baked-in Cargo/runtime config, so the
//! script only emits rerun hints that make a clean rebuild deterministic.
//! Kernel sources and the linker script are owned by the boot-asm milestone.

fn main() {
    // Rerun this script whenever a kernel source, an assembly file, or a linker
    // layout changes, mirroring future milestones without glue here.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
}
