# vendored dependencies

This directory is the offline source registry for the Glenda workspace
(see `config.toml`). It is intentionally empty: the kernel and xtask crates
have no third-party runtime dependencies, so a clean offline `cargo xtask
build` resolves everything locally.

When a new dependency is needed, vendor it and commit the generated tree:

    cargo vendor --versioned-dirs

Then keep `target/riscv64gc-unknown-none-elf/debug/kernel` reproducible across
`cargo clean -p kernel` rebuilds.
