//! Build script for RustDupe
//!
//! This build script handles platform-specific configuration:
//! - Windows: Embeds the application manifest for long path support (>260 chars)
//!
//! # Windows Long Path Support
//!
//! By default, Windows limits file paths to 260 characters (MAX_PATH).
//! This causes issues when scanning directories like `node_modules` that
//! often have deeply nested paths exceeding this limit.
//!
//! The manifest file (`rustdupe.manifest`) includes `longPathAware=true`
//! which, combined with the Windows 10 v1607+ registry setting, enables
//! paths up to 32,767 characters.
//!
//! # Usage
//!
//! This script runs automatically during `cargo build`. No manual intervention
//! is required. On non-Windows platforms, the script does nothing.

fn main() {
    // Only compile and embed the manifest on Windows
    #[cfg(windows)]
    {
        // Use embed-resource to compile the .rc file which references the manifest
        // The .rc file uses RT_MANIFEST resource type to embed the XML manifest
        embed_resource::compile("rustdupe.rc", embed_resource::NONE);

        // Instruct Cargo to re-run this build script if either file changes
        println!("cargo:rerun-if-changed=rustdupe.rc");
        println!("cargo:rerun-if-changed=rustdupe.manifest");
    }

    // On non-Windows platforms, we don't need to do anything special
    #[cfg(not(windows))]
    {
        // This block intentionally left empty
        // The build script exits successfully without embedding any resources
    }
}
