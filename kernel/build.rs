fn main() {
    println!("cargo:rerun-if-changed=src/boot.S");
    println!("cargo:rerun-if-changed=src/linker.ld");
    cc::Build::new().file("src/boot.S").flag("-march=rv64gc").flag("-mabi=lp64d").compile("boot");
}
