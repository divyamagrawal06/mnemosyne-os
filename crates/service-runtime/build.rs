fn main() {
    let source = "../../workloads/kv/service.wat";
    let bytes = wat::parse_file(source).expect("valid fixed Wasm fixture");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("kv-service.wasm"), bytes).unwrap();
    println!("cargo:rerun-if-changed={source}");
}
