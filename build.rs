fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        // Use relative path without leading slash
        res.set_icon("src/resources/icons/app.ico")
            .set("ProductName", "Mobile Dev IDE")
            .set("FileDescription", "Mobile Development IDE")
            .set("LegalCopyright", "Copyright © 2024 Your Name");

        // Verify path (already correct)
        let icon_path = std::path::Path::new("src/resources/icons/app.ico");
        if !icon_path.exists() {
            panic!("Icon file not found at: {}", icon_path.display());
        }

        res.compile().expect("Failed to compile Windows resources");
    }
    
    println!("cargo:rerun-if-changed=src/resources/icons/");
}