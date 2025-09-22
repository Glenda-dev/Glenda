// The Build File for Glenda

fn main() {
    println!("cargo:rerun-if-changed=boot.S");
    cc::Build::new()
        .file("./src/boot.S")
        .flag("-march=rv64gc")
        .flag("-mabi=lp64d")
        .compile("boot");
}
