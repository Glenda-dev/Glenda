# Glenda OS - AI Coding Instructions

You are working on **Glenda**, a research microkernel operating system written in Rust for RISC-V (rv64gc). It combines **seL4** design principles (capabilities, strict microkernel) with **Plan 9** concepts (namespaces, file-oriented).

## 1. Project Structure & Architecture

The workspace consists of distinct components with strict boundaries:

- **Kernel (\`kernel/\`)**: \`no_std\`. The core microkernel.
  - **Capabilities**: \`src/cap/\` (CNode, Capability, Rights, invocation). Resources are capabilities.
  - **IPC**: \`src/ipc/\` (Endpoint, Message). Synchronous IPC.
  - **Memory**: \`src/mem/\` (PageTable, Frame Allocator). Custom allocator.
  - **Traps**: \`src/trap/\` (Syscall dispatch, IRQ, Exceptions).
  - **Process**: \`src/proc/\` (Scheduler, Context).
- **Userspace Library (\`lib/libglenda-rs/\`)**: The standard library for apps/drivers.
  - Wraps syscalls (\`src/syscall.rs\`).
  - Provides runtime support (\`crt0\`, heap).
- **Services (\`service/\`)**: Userspace servers.
  - \`9ball\`: Init/Root task.
  - \`factotum\`: Service manager.
  - \`unicorn\`: Device manager.
- **Drivers (\`drivers/\`)**: Userspace drivers (e.g., \`ns16550a\`, \`virtio\`).
- **Build System (\`xtask/\`)**: Rust-based build/run tooling.

## 2. Workflows & Commands

**ALWAYS** use \`cargo xtask\` instead of \`cargo build\` directly for the kernel/system.

### Build & Run
- **Build System**: \`cargo xtask build\` (Compile kernel & services defined in config).
- **Run QEMU**: \`cargo xtask run\` (Builds, creates fs, boots QEMU).
  - Options: \`--cpus <N>\`, \`--mem <SIZE>\`, \`--display <TYPE>\`.
- **Debug (GDB)**: \`cargo xtask gdb\` (Starts QEMU paused on port 1234).
- **Generate FS**: \`cargo xtask mkfs\` (Creates \`disk.img\`).

### Testing
- Integration tests are defined in \`test.toml\`.
- To run tests (if the \`test\` command is unavailable/custom):
  - Check \`test.toml\` for test definitions.
  - Use \`cargo xtask --config test.toml run\` to boot into test/verification mode.
  - (Note: \`README.md\` mentions \`cargo xtask test\`, but verify availability in \`xtask/src/main.rs\`).

## 3. Development Conventions

### Systems Programming
- **\`no_std\`**: Kernel and services do not use the standard library.
- **Memory**:
  - **Kernel**: strict manual compilation of page tables/frames.
  - **Userspace**: Use \`extern crate alloc\` via \`libglenda-rs\`.
- **Panics**: Kernel panics halt the system (\`panic_handler\` in \`main.rs\`). Userspace panics abort the thread.

### Kernel Patterns
- **Logging**: Use \`printk!\` macro (kernel-only).
- **Capabilities**: All resource access (memory, IRQ, endpoints) MUST go through capability lookups (\`tcb.cap_lookup(cptr)\`).
- **Syscall Dispatch**:
  1. Trap handler (\`trap/mod.rs\`) calls \`syscall::dispatch\`.
  2. \`syscall::dispatch\` looks up capability -> checks rights -> calls \`invoke::dispatch\`.

### Adding Features
- **New Syscall**:
  1. Add constant in \`lib/libglenda-rs/include/glenda.h\`.
  2. Implement handler in \`kernel/src/trap/invoke.rs\` (or specific resource file).
  3. Expose wrapper in \`lib/libglenda-rs/src/syscall.rs\`.
- **New Service/Driver**:
  1. Create crate in \`service/\` or \`drivers/\`.
  2. Add entry to \`config/manifest.json\` (for default boot) or create a test config.
  3. Ensure it depends on \`libglenda-rs\` for syscalls/runtime.

## 4. Integration
- **Manifest**: \`config/manifest.json\` controls which services/drivers are packed into the boot image.
- **IPC**: Primary communication mechanism. Services expose endpoints.
- **DTB**: Device Tree passed by OpenSBI is parsed in \`kernel/src/dtb.rs\` to detect hardware.