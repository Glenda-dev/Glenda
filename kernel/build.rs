fn main() {
    println!("cargo:rerun-if-changed=src/boot.S");
    println!("cargo:rerun-if-changed=src/trap/vector.S");
    println!("cargo:rerun-if-changed=src/init/sbi.S");
    println!("cargo:rerun-if-changed=src/user/enter.S");
    println!("cargo:rerun-if-changed=src/linker.ld");

    // Service Generator
    // TODO: 重构这个东西
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let service_bin = std::path::Path::new(&manifest_dir).join("..").join("service").join("hello.bin");
    println!("cargo:rerun-if-changed={}", service_bin.display());
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_file = std::path::Path::new(&out_dir).join("user_payload.rs");
    if service_bin.exists() {
        let content = format!(
            "pub const USER_PAYLOAD: &[u8] = include_bytes!(\"{}\");\npub const HAS_USER_PAYLOAD: bool = true;\n",
            service_bin.display()
        );
        std::fs::write(&out_file, content).unwrap();
    } else {
        let content = String::from(
            "pub const USER_PAYLOAD: &[u8] = &[];\npub const HAS_USER_PAYLOAD: bool = false;\n",
        );
        std::fs::write(&out_file, content).unwrap();
    }

    cc::Build::new()
        .file("src/boot.S")
        .file("src/trap/vector.S")
        .file("src/init/sbi.S")
        .file("src/user/enter.S")
        .flag("-march=rv64gc")
        .flag("-mabi=lp64d")
        .compile("boot");
}
