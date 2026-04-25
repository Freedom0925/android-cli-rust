use std::path::PathBuf;
use std::env;
use anyhow::Result;

/// Environment configuration for Android CLI
pub struct EnvConfig {
    /// Android SDK home path
    pub android_home: Option<PathBuf>,
    /// Android CLI storage path
    pub cli_storage: PathBuf,
    /// Android user home (.android)
    pub android_user_home: PathBuf,
    /// Proxy URL (optional)
    pub proxy: Option<String>,
    /// SDK repository URL
    pub sdk_repo_url: String,
    /// SDK index URL
    pub sdk_index_url: String,
    /// Default channel
    pub default_channel: String,
    /// Disable metrics
    pub no_metrics: bool,
    /// Force license acceptance
    pub force_licenses: bool,
    /// Verbose output
    pub verbose: bool,
}

impl EnvConfig {
    /// Detect configuration from environment variables and defaults
    pub fn detect() -> Result<Self> {
        let user_home = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"));

        let android_user_home = user_home.join(".android");

        // ANDROID_HOME or ANDROID_SDK_ROOT
        let android_home = env::var("ANDROID_HOME")
            .ok()
            .or_else(|| env::var("ANDROID_SDK_ROOT").ok())
            .map(PathBuf::from);

        // CLI storage path
        let cli_storage_env = env::var("ANDROID_CLI_STORAGE").ok();
        let cli_storage = cli_storage_env
            .map(PathBuf::from)
            .unwrap_or_else(|| android_user_home.join("cli"));

        // Proxy
        let proxy = env::var("ANDROID_CLI_PROXY")
            .ok()
            .or_else(|| env::var("HTTP_PROXY").ok())
            .or_else(|| env::var("HTTPS_PROXY").ok());

        // SDK URLs
        let sdk_repo_url = env::var("ANDROID_SDK_REPO_URL")
            .unwrap_or_else(|_| "https://dl.google.com/android/repository".to_string());

        let sdk_index_url = env::var("ANDROID_SDK_INDEX_URL")
            .unwrap_or_else(|_| format!("{}/package_list.binpb", sdk_repo_url));

        // Default channel
        let default_channel = env::var("ANDROID_CLI_CHANNEL")
            .unwrap_or_else(|_| "stable".to_string());

        // Metrics
        let no_metrics = env::var("ANDROID_CLI_NO_METRICS")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        // Force licenses
        let force_licenses = env::var("ANDROID_CLI_FORCE_LICENSES")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        // Verbose
        let verbose = env::var("ANDROID_CLI_VERBOSE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        Ok(Self {
            android_home,
            cli_storage,
            android_user_home,
            proxy,
            sdk_repo_url,
            sdk_index_url,
            default_channel,
            no_metrics,
            force_licenses,
            verbose,
        })
    }

    /// Get default SDK path based on platform
    pub fn default_sdk_path(&self) -> PathBuf {
        let platform = env::consts::OS;

        match platform {
            "macos" => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join("Library/Android/sdk"),
            "linux" => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join("Android/Sdk"),
            "windows" => env::var("LOCALAPPDATA")
                .map(|p| PathBuf::from(p).join("Android/Sdk"))
                .ok()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("/"))
                        .join("Android/Sdk")
                }),
            _ => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join("Android/Sdk"),
        }
    }

    /// Get effective SDK path
    pub fn sdk_path(&self) -> PathBuf {
        self.android_home.clone()
            .unwrap_or_else(|| self.default_sdk_path())
    }

    /// Get ADB path
    pub fn adb_path(&self) -> PathBuf {
        self.sdk_path().join("platform-tools").join("adb")
    }

    /// Get emulator path
    pub fn emulator_path(&self) -> PathBuf {
        let new_path = self.sdk_path().join("emulator").join("emulator");
        if new_path.exists() {
            new_path
        } else {
            self.sdk_path().join("tools").join("emulator")
        }
    }

    /// Print current configuration
    pub fn print(&self) {
        println!("Android CLI Configuration:");
        println!("  SDK Path: {}", self.sdk_path().display());
        println!("  CLI Storage: {}", self.cli_storage.display());
        println!("  Android User Home: {}", self.android_user_home.display());
        println!("  SDK Repository: {}", self.sdk_repo_url);
        println!("  Default Channel: {}", self.default_channel);
        if let Some(proxy) = &self.proxy {
            println!("  Proxy: {}", proxy);
        }
        println!("  No Metrics: {}", self.no_metrics);
        println!("  Force Licenses: {}", self.force_licenses);
    }
}

/// Environment variable names for documentation
pub const ENV_VARS: &[(&str, &str)] = &[
    ("ANDROID_HOME", "Path to Android SDK"),
    ("ANDROID_SDK_ROOT", "Alternative path to Android SDK"),
    ("ANDROID_CLI_STORAGE", "Path to CLI storage directory"),
    ("ANDROID_CLI_PROXY", "Proxy URL for HTTP requests"),
    ("HTTP_PROXY", "Standard HTTP proxy"),
    ("HTTPS_PROXY", "Standard HTTPS proxy"),
    ("ANDROID_SDK_REPO_URL", "SDK repository base URL"),
    ("ANDROID_SDK_INDEX_URL", "SDK index URL (package_list.binpb)"),
    ("ANDROID_CLI_CHANNEL", "Default channel (stable/beta/canary)"),
    ("ANDROID_CLI_NO_METRICS", "Disable metrics (1/true)"),
    ("ANDROID_CLI_FORCE_LICENSES", "Auto-accept licenses (1/true)"),
    ("ANDROID_CLI_VERBOSE", "Enable verbose output (1/true)"),
];
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_config_detect() {
        // Clear any existing env vars for clean test
        env::remove_var("ANDROID_HOME");
        env::remove_var("ANDROID_SDK_ROOT");

        let config = EnvConfig::detect().unwrap();
        assert!(!config.cli_storage.to_str().unwrap().is_empty());
    }

    #[test]
    fn test_default_sdk_path() {
        let config = EnvConfig::detect().unwrap();
        let path = config.default_sdk_path();
        assert!(path.to_str().unwrap().contains("Android"));
    }

    #[test]
    fn test_sdk_path_with_env() {
        // Set env var temporarily
        env::set_var("ANDROID_HOME", "/custom/sdk/path");
        let config = EnvConfig::detect().unwrap();
        assert_eq!(config.sdk_path().to_str().unwrap(), "/custom/sdk/path");
        env::remove_var("ANDROID_HOME");
    }

    #[test]
    fn test_adb_path() {
        let config = EnvConfig::detect().unwrap();
        let adb_path = config.adb_path();
        assert!(adb_path.to_str().unwrap().contains("platform-tools"));
        assert!(adb_path.to_str().unwrap().contains("adb"));
    }

    #[test]
    fn test_emulator_path() {
        let config = EnvConfig::detect().unwrap();
        let emu_path = config.emulator_path();
        assert!(emu_path.to_str().unwrap().contains("emulator"));
    }

    #[test]
    fn test_default_channel() {
        let config = EnvConfig::detect().unwrap();
        assert_eq!(config.default_channel, "stable");
    }
}
