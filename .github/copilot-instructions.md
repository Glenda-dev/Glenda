# Glenda OS - AI Coding Instructions

You are working on **Glenda**, a research microkernel operating system written in Rust for RISC-V (rv64gc). It combines **seL4** design principles (capabilities, strict microkernel) with **Plan 9** concepts.

## 1. Project Architecture

The workspace strictly separates kernel and userspace components:

- **Kernel** (`kernel/`): `no_std`. The core microkernel.
  - **Capabilities**: `src/cap/`. All resources (memory, IRQ, endpoints) are capabilities accessed via mechanism-only lookups.
  - **IPC**: `src/ipc/`. Synchronous IPC using **UTCB** (User Thread Control Block) for message passing (registers + buffer).
  - **Traps**: `src/trap/`. Handles syscalls (`invoke.rs`), IRQs, and exceptions.
- **Userspace Library** (`lib/libglenda-rs/`): The standard library for apps.
  - **Runtime**: `crt0` entry, heap allocation, and syscall wrappers.
  - **Components**: `src/ipc/utcb.rs` defines the shared memory structure for IPC.
- **Services** (`service/`):
  - **factotum**: The **Root Task** (init process) in default config. Manages service startup.
  - **unicorn**: Device manager.
  - **nineball**: System server (Plan 9-like functionality).
- **Build System** (`xtask/`): Rust-based CLI for build/run automation.

## 2. Workflows & Commands

**DO NOT** use `cargo build` directly. Use `cargo xtask`.

### Essential Commands
- **Build**: `cargo xtask build` (Compiles kernel & services per `config.toml`).
- **Run (QEMU)**: `cargo xtask run` (Builds, creates FS, boots).
- **Debug**: `cargo xtask gdb` (Starts QEMU paused on port 1234).
- **Filesystem**: `cargo xtask mkfs` (Generates `disk.img`).

### Testing
- Tests are defined as alternative system configurations.
- **Run Tests**: `cargo xtask --config test.toml run`.
  - This typically replaces the root task with a test binary (e.g., `examples/hello`).
  - See `test.toml` for the definition of the test environment.

## 3. Development Conventions

### Systems Programming
- **Kernel**: `no_std`, manual page table management. Uses `printk!` for logging.
- **Services**: `no_std`, but link `extern crate alloc`. Uses `glenda::log!` or `println!` (via `libglenda-rs`).
- **Panics**: Kernel panics halt the system (`panic_handler` in `main.rs`). Service panics abort the thread.

### IPC & Syscalls
- **Mechanism**: Syscalls dispatch via `sys_invoke`. Arguments are marshaled into the **UTCB**.
  - **UTCB**: Located at fixed virtual address (`UTCB_VA`). Contains `MsgTag`, registers (`mrs_regs`), and a ring buffer (`ipc_buffer`).
- **Pattern**:
  1. Service creates an `Endpoint`.
  2. Client `Call`s the endpoint, writing data/caps to UTCB.
  3. Kernel transfers data from Sender UTCB to Receiver UTCB.

### Configuration
- **Manifests**:
  - `config.toml`: Defines the **build artifacts** (kernel, root task, services) for the system image.
  - `config/manifest.json`: Runtime configuration (parsed by services like `factotum`) to identify available drivers/binaries in the FS.

## 4. Integration Points
- **Adding a Syscall**:
  1. Define in `lib/libglenda-rs/include/glenda.h` (constants).
  2. Implement in `kernel/src/trap/invoke.rs`.
  3. Wrap in `lib/libglenda-rs/src/syscall.rs`.
- **New Service**:
  1. Create crate in `service/`.
  2. Add to `config.toml` (to include in build/image).
  3. Ensure it uses `libglenda-rs` for runtime support.
