# GEMINI.md - Glenda Project Context

## Project Overview
**Glenda** is a cross-architecture research microkernel operating system written in Rust. It aims to combine the formal design principles of **seL4** (strict capability-based isolation) with the distributed philosophy of **Plan 9** (Everything is a file & Private Namespaces).

### Architecture
- **Microkernel (`kernel/`)**: Implements minimal mechanisms: Capability management (CNode/Untyped), IPC (via UTCB), Address Space management (VSpace), and Preemptive Scheduling.
- **Root Task (Warren - `service/warren/`)**: The first user-mode process. It receives all initial system resources (Untyped memory) from the kernel and is responsible for bootstrapping the rest of the system.
- **System Services (`service/`)**:
    - `Nineball`: Orchestrator and service discovery.
    - `Fossil`: Global namespace and file system manager.
    - `Gopher`: User-mode network stack.
    - `Unicorn`: Driver management framework.
- **User-mode Drivers (`drivers/`)**: Hardware drivers (VirtIO, UART, RTC, etc.) run in their own address spaces.
- **Libraries (`lib/`)**:
    - `libglenda-rs`: The core runtime for user-mode Rust services.
    - `musl-glenda`: A port of the musl C library for POSIX compatibility.

## Building and Running
The project uses a custom `xtask` automation system. **Do not use `cargo build` at the root.**

### Key Commands
- **Build**: `cargo xtask build` - Compiles the kernel, services, and packages the rootfs.
- **Run**: `cargo xtask run` - Boots the system in QEMU.
- **Debug**: `cargo xtask gdb` - Starts QEMU in wait-for-gdb mode.
- **Testing**: `cargo xtask check` - Runs `cargo check` across all workspace members.
- **Disk Management**:
    - `cargo xtask mount`: Mounts the `disk.img` to the `mnt/` directory (requires `sudo`).
    - `cargo xtask umount`: Unmounts the image.
- **Custom Config**: `cargo xtask --config config/hello.toml run` - Runs with a specific configuration.

## Development Conventions
- **Microkernel Minimalism**: Logic should reside in user-mode services unless it strictly requires kernel privileges (resource management, context switching).
- **Capability-Based Security**: Access to resources (memory, IPC endpoints, device IRQs) must be mediated via capabilities.
- **Error Handling**: Use the internal `Error` type defined in the kernel and libraries.
- **Async/Sync**: The kernel uses a synchronous IPC model; high-level services may use async patterns via `libglenda-rs`.
- **Target Architecture**: Primarily targeting **RISC-V 64-bit** (S-mode).

## Project Structure
- `kernel/`: Core microkernel source.
- `service/`: System services and root tasks.
- `drivers/`: User-mode hardware drivers.
- `fs/`: File system implementations (FAT, EXT, InitRD).
- `lib/`: Standard libraries and POSIX compatibility layers.
- `platform/`: Architecture-specific Device Tree (DTB) and ACPI files.
- `xtask/`: Build system and automation logic.
- `config/`: System initialization and service configuration files (JSON/TOML).
- `mnt/`: Default mount point for the disk image.
