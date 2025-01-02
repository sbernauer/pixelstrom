use std::io::Result;

fn main() -> Result<()> {
    // We could also source protoc by building it:
    // https://docs.rs/prost-build/latest/prost_build/#compiling-protoc-from-source
    prost_build::compile_protos(&["src/proto/WebSocketMessage.proto"], &["src/"])?;

    // let javascript_protobuf_bindings_dir = format!("{}/static/proto/", env!("CARGO_MANIFEST_DIR"));

    // create_dir_all(&javascript_protobuf_bindings_dir)
    //     .expect(&format!("Failed to create javascript protobuf bindings directory ({javascript_protobuf_bindings_dir})"));

    // let panic_message =
    //     "Failed to generate javascript code from protobuf schema. Is protoc-gen-js installed?";
    // if !Command::new("protoc")
    //     .args([
    //         &format!(
    //             // "--js_out=import_style=commonjs,binary:{}/static/proto/",
    //             // "--js_out={}/static/proto/",
    //             "--js_out=import_style=es6:{}/static/proto/",
    //             env!("CARGO_MANIFEST_DIR")
    //         ),
    //         "src/proto/WebSocketMessage.proto",
    //     ])
    //     .status()
    //     .expect(panic_message)
    //     .success()
    // {
    //     panic!("{}", panic_message);
    // };

    Ok(())
}
