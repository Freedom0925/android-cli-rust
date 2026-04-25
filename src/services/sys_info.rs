use std::path::PathBuf;

/// Platform enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Mac,
    Windows,
}

/// CPU Architecture enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArch {
    X86_64,
    Aarch64,
}

/// System information service
pub struct SysInfoService {
    pub platform: Platform,
    pub cpu_arch: CpuArch,
    pub user_home: PathBuf,
    pub android_user_home: PathBuf,
}

impl SysInfoService {
    /// Detect current system information
    pub fn detect() -> Self {
        let platform = match std::env::consts::OS {
            "linux" => Platform::Linux,
            "macos" => Platform::Mac,
            "windows" => Platform::Windows,
            _ => Platform::Linux, // Default fallback
        };

        let cpu_arch = match std::env::consts::ARCH {
            "x86_64" | "amd64" => CpuArch::X86_64,
            "aarch64" | "arm64" => CpuArch::Aarch64,
            _ => CpuArch::X86_64, // Default fallback
        };

        let user_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let android_user_home = user_home.join(".android");

        Self {
            platform,
            cpu_arch,
            user_home,
            android_user_home,
        }
    }

    /// Get default SDK path based on platform
    pub fn default_sdk_path(&self) -> PathBuf {
        // Check ANDROID_HOME environment variable first
        if let Ok(android_home) = std::env::var("ANDROID_HOME") {
            return PathBuf::from(android_home);
        }

        // Check ANDROID_SDK_ROOT (alternative env var)
        if let Ok(android_sdk_root) = std::env::var("ANDROID_SDK_ROOT") {
            return PathBuf::from(android_sdk_root);
        }

        // Use platform-specific defaults
        match self.platform {
            Platform::Mac => self.user_home.join("Library/Android/sdk"),
            Platform::Linux => self.user_home.join("Android/Sdk"),
            Platform::Windows => {
                std::env::var("LOCALAPPDATA")
                    .map(|p| PathBuf::from(p).join("Android/Sdk"))
                    .ok()
                    .unwrap_or_else(|| self.user_home.join("Android/Sdk"))
            }
        }
    }

    /// Get ADB executable path
    pub fn adb_path(&self, sdk_path: &PathBuf) -> PathBuf {
        let suffix = if self.platform == Platform::Windows { ".exe" } else { "" };
        sdk_path.join("platform-tools").join(format!("adb{}", suffix))
    }

    /// Get emulator executable path
    pub fn emulator_path(&self, sdk_path: &PathBuf) -> PathBuf {
        let suffix = if self.platform == Platform::Windows { ".exe" } else { "" };
        sdk_path.join("emulator").join(format!("emulator{}", suffix))
    }

    /// Get CLI storage path (.android/cli/)
    pub fn cli_storage_path(&self) -> PathBuf {
        self.android_user_home.join("cli")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sys_info_detect() {
        let sys_info = SysInfoService::detect();
        // Should detect something valid
        assert!(sys_info.user_home.exists() || sys_info.user_home == PathBuf::from("/"));
    }
}