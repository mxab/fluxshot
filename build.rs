//! Windows only: embed the app icon and version info into fluxshot.exe so
//! Explorer, the Start menu and the installer show them.

fn main() {
    println!("cargo:rerun-if-changed=assets/fluxshot.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/fluxshot.ico")
            .set("ProductName", "FluxShot")
            .set("FileDescription", "FluxShot screenshot tool");
        match res.compile() {
            Ok(()) => {}
            // Building on Windows (as releases do): a missing icon is a bug.
            Err(e) if std::env::var("HOST").is_ok_and(|h| h.contains("windows")) => {
                panic!("embedding Windows resources: {e}")
            }
            // Cross-checks from other hosts usually lack a resource compiler.
            Err(e) => println!("cargo:warning=could not embed Windows resources: {e}"),
        }
    }
}
