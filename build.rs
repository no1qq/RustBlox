fn main() {
    println!("cargo:rerun-if-changed=assets/rustblox.ico");
    println!("cargo:rerun-if-changed=assets/thewatcher.ico");
    println!("cargo:rerun-if-changed=assets/rustblox.manifest");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set("ProductName", "RustBlox");
        resource.set("FileDescription", "RustBlox desktop client for Roblox");
        resource.set("LegalCopyright", "Licensed under MIT");
        resource.set("OriginalFilename", "RustBlox.exe");

        if std::path::Path::new("assets/rustblox.ico").exists() {
            resource.set_icon("assets/rustblox.ico");
        }
        if std::path::Path::new("assets/rustblox.manifest").exists() {
            resource.set_manifest_file("assets/rustblox.manifest");
        }

        if let Err(err) = resource.compile() {
            println!("cargo:warning=windows resources were not embedded: {err}");
        }
    }
}
