fn main() {
    // Watch for the generated modules blob in workspace target
    println!("cargo:rerun-if-changed=src/linker.ld");
}
