fn main() {
    println!("cargo:rerun-if-changed=src/boot.S");
    println!("cargo:rerun-if-changed=src/trap/vector.S");
    println!("cargo:rerun-if-changed=src/init/sbi.S");
    println!("cargo:rerun-if-changed=src/linker.ld");
    cc::Build::new()
        .file("src/boot.S")
        .file("src/trap/vector.S")
        .file("src/init/sbi.S")
        .flag("-march=rv64gc")
        .flag("-mabi=lp64d")
        .compile("boot");
}
