fn main() {
    assert_eq!(std::env::var("TARGET").unwrap(), "x86_64-unknown-none");
    let linker = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("linker-x86_64.ld");
    println!("cargo:rustc-link-arg=-T{}", linker.display());
    println!("cargo:rerun-if-changed=linker-x86_64.ld");
}
