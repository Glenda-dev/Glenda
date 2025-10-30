fn main() {
    println!("cargo:rerun-if-changed=src/asm/boot.S");
    println!("cargo:rerun-if-changed=src/asm/vector.S");
    println!("cargo:rerun-if-changed=src/asm/sbi.S");
    println!("cargo:rerun-if-changed=src/asm/enter.S");
    println!("cargo:rerun-if-changed=src/asm/switch.S");
    println!("cargo:rerun-if-changed=src/linker.ld");
    cc::Build::new()
        .file("src/asm/boot.S")
        .file("src/asm/vector.S")
        .file("src/asm/sbi.S")
        .file("src/asm/enter.S")
        .file("src/asm/switch.S")
        .flag("-march=rv64gc")
        .flag("-mabi=lp64d")
        .compile("boot");
}
