//! Embeds the icon, version info and manifest into Windows executables.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/storagemap.ico");
    println!("cargo:rerun-if-changed=windows/storagemap.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/storagemap.ico")
        .set_manifest_file("windows/storagemap.manifest")
        .set("ProductName", "StorageMap")
        .set("FileDescription", "StorageMap disk space analyzer")
        .set("LegalCopyright", "StorageMap contributors");
    // Cross-checks without a resource compiler still build; release CI
    // runs on Windows where `rc.exe` is always present.
    if let Err(err) = res.compile() {
        println!("cargo:warning=Windows resources not embedded: {err}");
    }
}
