//! CLI context and system information

use anyhow::{Result, bail};
use std::path::PathBuf;

use android_cli::sdk::Channel;
use android_cli::sdk::protobuf::{Platform, Architecture};

/// Execution context for CLI commands
pub struct Context {
    pub sdk_path: PathBuf,
    pub sdk_index: String,
    pub sdk_url: String,
    pub sys_info: SysInfoService,
    pub no_metrics: bool,
}

impl Context {
    pub fn new(
        sdk_path: Option<String>,
        sdk_index: String,
        sdk_url: String,
        no_metrics: bool,
    ) -> Result<Self> {
        let sys_info = SysInfoService::detect();
        let sdk_path = sdk_path
            .map(PathBuf::from)
            .or_else(|| std::env::var("ANDROID_HOME").ok().map(PathBuf::from))
            .unwrap_or_else(|| sys_info.default_sdk_path());

        Ok(Self {
            sdk_path,
            sdk_index,
            sdk_url,
            sys_info,
            no_metrics,
        })
    }
}

/// System information service
pub struct SysInfoService {
    pub platform: Platform,
    pub arch: Architecture,
    pub user_home: PathBuf,
    pub android_user_home: PathBuf,
}

impl SysInfoService {
    pub fn detect() -> Self {
        let platform = Platform::current();
        let arch = Architecture::current();
        let user_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            platform,
            arch,
            user_home: user_home.clone(),
            android_user_home: user_home.join(".android"),
        }
    }

    pub fn default_sdk_path(&self) -> PathBuf {
        match self.platform {
            Platform::Mac => self.user_home.join("Library/Android/sdk"),
            Platform::Linux | Platform::Unspecified => self.user_home.join("Android/Sdk"),
            Platform::Windows => std::env::var("LOCALAPPDATA")
                .map(|p| PathBuf::from(p).join("Android/Sdk"))
                .ok()
                .unwrap_or_else(|| self.user_home.join("Android/Sdk")),
        }
    }

    pub fn cli_storage_path(&self) -> PathBuf {
        self.android_user_home.join("cli")
    }
}

/// Get channel from canary/beta flags (matches Kotlin getChannel)
pub fn get_channel_from_flags(canary: bool, beta: bool) -> Result<Channel> {
    if canary && beta {
        bail!("--canary and --beta flags cannot be set at the same time");
    }
    if canary {
        Ok(Channel::Canary)
    } else if beta {
        Ok(Channel::Beta)
    } else {
        Ok(Channel::Stable)
    }
}