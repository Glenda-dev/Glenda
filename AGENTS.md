# Repository Guidelines

## Project Structure & Module Organization
Glenda is a Rust microkernel workspace. Core kernel code lives in `kernel/src/`. User-space services are under `service/`, drivers under `drivers/`, file systems under `fs/`, shared code under `lib/`, and examples under `examples/`. Build automation is in `xtask/`; prefer extending it instead of adding ad hoc scripts. Configurations live in `config/`, platform data in `platform/`, and design notes in `docs/zh/`. Integration test crates are in `tests/ipc`, `tests/ping`, and `service/hutch/tests/suite`.

## Build, Test, and Development Commands
This project does not support plain `cargo build`; use the workspace task runner.

- `cargo xtask build`: build the configured kernel, services, drivers, and image artifacts.
- `cargo xtask run`: build and boot the default configuration in QEMU.
- `cargo xtask check`: run target-aware `cargo check` for the kernel, libraries, and configured services.
- `cargo xtask --config config/hello.toml run`: run a specific config, such as the hello example.
- `cargo xtask --config tests/ipc/config/ipc_test.toml run`: boot the IPC test; use `tests/ping/config/ping_test.toml` for networking.
- `cargo xtask gdb`: start QEMU paused for remote GDB debugging.
- `cargo xtask mount` / `cargo xtask umount`: mount or unmount rootfs at `mnt/`; these may invoke `sudo`.

## Coding Style & Naming Conventions
Rust code uses edition 2024 with `rustfmt.toml` enforcing Unix newlines and `use_small_heuristics = "Max"`. Run `cargo fmt --all` before submitting changes. Use snake_case for modules, functions, files, and package directories; use UpperCamelCase for types and traits. Keep crate names aligned with service or driver names, for example `service/gopher` and `drivers/virtio/net`.

## Testing Guidelines
Prefer `cargo xtask check` for fast validation because it applies the configured architecture target and profile. For runtime behavior, add or update a config under `tests/<name>/config/` and run it with `cargo xtask --config ... run`. Name test packages and outputs descriptively, such as `ipc-test-client` or `ping-test`.

## Commit & Pull Request Guidelines
Git history uses short, bracketed scopes such as `[chore] Sync deps`, `[kernel][arch] Update aarch64 support`, and `[test] Add ipc and ping tests`. Follow that style: start with one or more scopes, then an imperative summary. Pull requests should describe the affected subsystem, list commands run, link issues, and include boot logs or screenshots for QEMU-visible changes.

## Security & Configuration Tips
Do not commit generated files from `target/`, mounted contents from `mnt/`, local disk images, or machine-specific IDE state. Treat `config/*.toml` and test configs as public interface: document new boot arguments, services, files, or features.
