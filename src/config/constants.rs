//! Configuration constants for Android CLI
//!
//! Centralized URL and configuration values

/// SDK repository URLs
pub mod sdk {
    /// Default SDK index URL (protobuf format)
    pub const DEFAULT_SDK_INDEX_URL: &str =
        "https://dl.google.com/android/repository/package_list.binpb";

    /// Default SDK artifact repository URL
    pub const DEFAULT_SDK_URL: &str = "https://dl.google.com/android/repository";
}

/// ARM SDK fallback URLs (for Linux aarch64)
pub mod arm_sdk {
    /// GitHub repository for custom ARM SDK
    pub const GITHUB_REPO: &str = "HomuHomu833/android-sdk-custom";

    /// GitHub API URL for releases
    pub const GITHUB_API_URL: &str =
        "https://api.github.com/repos/HomuHomu833/android-sdk-custom/releases";

    /// Download URL pattern for ARM SDK archives
    /// Format: {version}/android-sdk-{arch}-linux-musl.tar.xz
    pub const DOWNLOAD_URL_PATTERN: &str =
        "https://github.com/HomuHomu833/android-sdk-custom/releases/download/{version}/android-sdk-{arch}-linux-musl.tar.xz";
}

/// Skills download URLs
pub mod skills {
    /// Skills download URL from GitHub releases
    pub const SKILLS_DOWNLOAD_URL: &str =
        "https://github.com/android/skills/releases/latest/download/android-skills.zip";
}

/// Knowledge base download URLs
pub mod docs {
    /// Knowledge base ZIP download URL
    pub const KB_ZIP_URL: &str = "https://dl.google.com/dac/dac_kb.zip";
}

// Re-export constants for easier access
pub use sdk::*;
pub use arm_sdk::*;
pub use skills::*;
pub use docs::*;