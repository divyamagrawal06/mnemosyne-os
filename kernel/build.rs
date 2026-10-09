// Adapted from the Limine Rust template (0BSD); see THIRD_PARTY.md.
fn main() {
    let target = std::env::var("TARGET").expect("Cargo supplies TARGET");
    assert_eq!(
        target, "x86_64-unknown-none",
        "build the kernel with tools/dev.py build"
    );
    let script = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("linker-x86_64.ld");
    println!("cargo:rustc-link-arg=-T{}", script.display());
    println!("cargo:rerun-if-changed=linker-x86_64.ld");
    let image = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../build/user-init.elf")
        .canonicalize()
        .expect("build user-init first with tools/dev.py");
    println!("cargo:rustc-env=MNEMOSYNE_USER_IMAGE={}", image.display());
    println!("cargo:rerun-if-changed={}", image.display());
}
