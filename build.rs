fn main() {
    // Tell Cargo to rerun build.rs if proto files change
    std::fs::read_dir("protos").unwrap().for_each(|entry| {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().ends_with(".proto") {
            println!(
                "cargo:rerun-if-changed=protos/{}",
                entry.file_name().to_string_lossy()
            );
        }
    });

    // Compile protobuf files
    prost_build::compile_protos(&["protos/sdk.proto", "protos/local.proto"], &["protos/"])
        .expect("Failed to compile protobuf files");
}
