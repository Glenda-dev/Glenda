# Glenda OS - AI Coding Instructions

You are working on **Glenda**, a microkernel operating system written in Rust for RISC-V (rv64gc). It combines design principles from seL4 (capabilities, microkernel) and Plan 9 (namespaces, file-oriented).

## 1. Project Architecture & Structure

The project is organized as a workspace with several key components:

- **Kernel (`kernel/`)**: The core microkernel.
  - `src/cap/`: Capability system (CNode, Capability, Rights) - seL4 inspired.
  - `src/ipc/`: Inter-Process Communication (Endpoint, Message).
  - `src/proc/`: Process and Thread management (Scheduler, Context).
  - `src/mem/`: Memory management (PageTable, Frame Allocator).
  - `src/trap/`: Exception handling.
  - `src/irq/`: Interrupt handling。
  - `src/main.rs`: Kernel entry point (`glenda_main`).

- **Services (`service/`)**: Userspace servers that provide OS functionality.
  - `9ball`: Init Manager.
  - `factotum`: Process and Resource Manager.
  - `unicorn`: Device Manager.

- **Drivers (`drivers/`)**: Userspace drivers.
  - `ns16550a`: UART driver.
  - `virtio`: VirtIO drivers (Block, Net, etc.).

- **Libraries (`lib/`)**: Shared code.
  - `libglenda-rs`: The standard library for userspace applications, providing syscall wrappers and runtime support.

- **Build System (`xtask/`)**: A Rust-based build system replacing Makefiles.

## 2. Critical Workflows

**Do not use `cargo build` directly for the kernel.** Use the `xtask` system.

### Build & Run
- **Build Kernel**: `cargo xtask build`
- **Run in QEMU**: `cargo xtask run` (Builds kernel + generates fs + runs QEMU)
- **Run Tests**: `cargo xtask test`
- **Debug (GDB)**: `cargo xtask gdb` (Starts QEMU in suspended state listening on port 1234)
- **Generate Filesystem**: `cargo xtask mkfs`

### Configuration
- **Release Mode**: Add `--release` flag (e.g., `cargo xtask --release run`).


## 3. Coding Conventions & Patterns

### Rust & System Programming
- **`no_std`**: The kernel and most services are `no_std`.
- **Unsafe Code**: Permitted for hardware interaction, raw pointer manipulation, and FFI. Always verify safety invariants.
- **Memory Management**:
  - Kernel uses a custom allocator.
  - Userspace relies on `libglenda-rs` for heap allocation.

### Kernel Specifics
- **Logging**: Use the `printk!` macro for kernel-level logging.
- **Capabilities**: Access control is capability-based. Resources are represented as capabilities in a CNode (Capability Node).
- **Entry Point**: The kernel starts at `_start` (assembly) which calls `glenda_main` (Rust).

### Userspace Services
- **Dependencies**: Services should depend on `libglenda-rs` for system interaction.
- **Structure**: Each service is a separate Cargo package in the `service/` directory.

## 4. Integration & Communication
- **IPC**: Processes communicate via IPC endpoints. Messages are passed using the `ipc` module in the kernel and syscall wrappers in `libglenda-rs`.
- **Syscalls**: Defined in `include/kernel/syscall/` and implemented in `kernel/src/trap/`.
- **Device Tree**: The kernel parses the Flattened Device Tree (DTB) passed by OpenSBI to discover hardware.

## 5. Common Tasks
- **Adding a Syscall**:
  1. Define the syscall number in `include/kernel/syscall/num.h` (or equivalent Rust file).
  2. Implement the handler in `kernel/src/trap/`.
  3. Expose it to userspace via `libglenda-rs`.
- **Adding a Driver**:
  1. Create a new crate in `drivers/`.
  2. Implement the driver logic using `libglenda-rs` for MMIO and interrupts.
  3. Register the driver in the system manifest or startup scripts.
