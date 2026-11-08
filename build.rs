use glib_build_tools::compile_resources;

fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("locate vendored protoc");
    // Cargo build scripts are single-threaded here, so setting the process
    // environment before invoking external resource tools is safe.
    unsafe { std::env::set_var("PROTOC", protoc) };

    // Compile Servo's built-in resources (HSTS preload list, public suffix
    // list, debugger script, ...) into a GResource bundle that the runner
    // registers as Servo's resource reader.
    compile_resources(
        &["resources"],
        "resources/gresource.xml",
        "resources.gresource",
    );

    // The generated IPC module is checked in as `servo_gtk/servo_ipc_generated.rs`.
    // This keeps Windows builds independent of a host `protoc` installation.
    println!("cargo:rerun-if-changed=servo_gtk/servo_ipc_generated.rs");
}
