# Glenda OS - AI Coding Instructions

You are working on **Glenda**, a research microkernel operating system written in Rust for RISC-V (rv64gc). It follows **seL4\u0027s** strict capability model combined with **Plan 9\u0027s** service-oriented design.

## 1. Top-Level Architecture

The system is strictly divided into **Kernel Space** and **User Space**:

### A. Kernel (`kernel/`)
- **Role**: Minimal logic. Only handles capability management, thread scheduling, and IPC message passing.
- **Entry**: `src/trap/syscall.rs` handles the `sys_invoke` trap.
- **Key Concepts**:
  - `Cap`: Everything is a capability (memory, endpoints, IRQ).
  - `CSpace`: Capability Space (per-thread table of capability slots).
  - `UTCB` (User Thread Control Block): Fixed memory region for IPC message registers/buffers.

### B. Userspace (`lib/`, `service/`)
- **Runtime**: `libglenda-rs` is the standard library.
  - `src/ipc/utcb.rs`: Defines the shared memory layout (`UTCB_VA`) for message passing.
  - `src/manager/`: High-level wrappers for `CSpaceManager`, `VSpaceManager`, `ResourceManager`.
- **Services**:
  - **warren** (`service/warren`): The **Root Task** (init). It parses `BootInfo`, manages global resources, and spawns other services via `Initrd`. It acts as the "Monitor".
  - **nineball**, **unicorn**: Feature services spawned by warren.

## 2. Development Workflow

**DO NOT** use `cargo build` directly. The project uses `xtask` to manage the complex build chain.

### Commands
| Action | Command | Description |
|--------|---------|-------------|
| **Build** | `cargo xtask build` | Compiles kernel & services defined in `config.toml`. |
| **Run** | `cargo xtask run` | Builds, generates filesystem image, and boots in QEMU. |
| **Test** | `cargo xtask --config test.toml run` | Runs specific test configuration. |
| **Debug** | `cargo xtask gdb` | Starts QEMU paused on port 1234. |
| **Clean** | `cargo xtask clean` | Cleans target artifacts. |

### Configuration (`config.toml`)
- Defines which services are packed into the `disk.img` or `initrd`.
- To add a new service, you must add it to the `[services]` list in `config.toml`.

## 3. Coding Conventions & Patterns

### General
- **No Std**: Both kernel and services are `no_std`. Services link internal `alloc`.
- **Logging**:
  - **Kernel**: Use `printk!`.
  - **Services**: Use `glenda::println!` or the `log!` macro defined in `main.rs` (wraps `glenda::println!`).

### IPC & System Calls
The term "syscall" in Glenda usually refers to **Capability Invocation**:
1. **Low-Level**: User calls `sys_invoke(cptr, method, ...)`.
2. **Kernel**: Map `cptr` to a Capability -> `dispatch()` in `kernel/src/trap/syscall.rs`.
3. **High-Level**: Standard OS calls (e.g., `sbrk`, `exit`) are implemented in `libglenda-rs/src/sys/` as **IPC messages** sent to `warren` (via `MONITOR_CAP`).

### Defining a New Capability/Syscall
1. **Kernel Side**: Implement `Dispatch` trait for the capability in `kernel/src/cap/`.
2. **User Side**: Add method constants in `libglenda-rs/src/protocol/`.
3. **Wrapper**: Implement the invocation wrapper in `libglenda-rs/src/cap/`.

## 4. Key Data Structures
- **UTCB**: Located at `UTCB_VA`. Contains `MsgTag` and `mrs_regs` (Message Registers) for fast IPC arguments.
- **BootInfo**: Located at `BOOTINFO_VA`. Parsed by `warren` to discover `initrd` location and free memory regions.

## 5. Critical Files
- `kernel/src/trap/syscall.rs`: The nexus of all system calls.
- `lib/libglenda-rs/src/ipc/utcb.rs`: The contract between Kernel and User for data transfer.
- `service/warren/src/main.rs`: System initialization logic.
