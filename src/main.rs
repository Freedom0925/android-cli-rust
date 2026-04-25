use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;

use android_cli::sdk::{SdkManager, Channel};
use android_cli::emulator::AvdManager;
use android_cli::adb::AdbService;
use android_cli::skills::SkillManager;
use android_cli::template::{TemplateProcessor, DeviceTemplates};
use android_cli::update::Updater;
use android_cli::docs::DocsCLI;
use android_cli::create::TemplateEngineRunner;
use android_cli::screen::{ScreenCommand, ResolveCommand};
use android_cli::layout::LayoutCommand;
use android_cli::describe::DescribeCLI;

/// Android CLI - Pure Rust implementation
#[derive(Parser)]
#[command(name = "android")]
#[command(version = "0.1.0")]
#[command(about = "Android development tools CLI")]
#[command(long_about = "Android CLI provides tools for SDK management, emulator control, device interaction, and more.")]
#[command(disable_help_subcommand = true)]
struct Cli {
    /// Path to Android SDK
    #[arg(long, global = true)]
    sdk: Option<String>,

    /// Disable metrics collection (hidden)
    #[arg(long, global = true, hide = true)]
    no_metrics: bool,

    /// SDK index URL (hidden)
    #[arg(long, global = true, hide = true, default_value = "https://dl.google.com/android/repository/package_list.binpb")]
    sdk_index: String,

    /// SDK artifact URL (hidden)
    #[arg(long, global = true, hide = true, default_value = "https://dl.google.com/android/repository")]
    sdk_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// SDK package management
    Sdk {
        #[command(subcommand)]
        command: SdkCommands,
    },
    /// Emulator management
    Emulator {
        #[command(subcommand)]
        command: EmulatorCommands,
    },
    /// Device management (requires ADB) - Hidden, use 'run' for device operations
    #[command(hide = true)]
    Device {
        #[command(subcommand)]
        command: DeviceCommands,
    },
    /// Run APK on device (Deploy an Android Application) (Deploy an Android Application)
    Run {
        /// The paths to the APKs
        #[arg(long)]
        apks: Vec<String>,

        /// The device serial number
        #[arg(long)]
        device: Option<String>,

        /// The component type (ACTIVITY, SERVICE, etc.)
        #[arg(long, default_value = "activity")]
        type_: String,

        /// The activity name
        #[arg(long)]
        activity: Option<String>,

        /// Run in debug mode
        #[arg(long)]
        debug: bool,
    },
    /// AI Agent Skills management
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },
    /// Template processing - Hidden, use 'create' for project creation
    #[command(hide = true)]
    Template {
        /// Process template file or directory
        #[arg(long)]
        process: Option<String>,

        /// Device profile for template variables
        #[arg(long)]
        profile: Option<String>,

        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Initialize Android CLI environment and install bundled skills
    Init {
        /// Force reinstall bundled skills
        #[arg(long)]
        force: bool,
    },
    /// Describe SDK packages or analyze Android project structure
    Describe {
        /// Project directory to analyze (if provided, analyzes Gradle project)
        #[arg(long = "project_dir")]
        project_dir: Option<String>,
    },
    /// Documentation search and fetch
    Docs {
        #[command(subcommand)]
        command: Option<DocsCommands>,
    },
    /// Self-update the CLI
    Update {
        /// URL to download update from
        #[arg(long)]
        url: Option<String>,
    },
    /// Print environment information
    Info {
        /// Specific field to print (sdk-path, user-home, android-home, platform, arch, version)
        field: Option<String>,
    },
    /// Create new Android project from template
    Create {
        /// The name of the application (e.g. 'My Application') - REQUIRED
        #[arg(long)]
        name: String,
        /// The destination project directory path (default is '.')
        #[arg(long, short = 'o', default_value = ".")]
        output: String,
        /// The 'minSdk' supported by the application
        #[arg(long)]
        min_sdk: Option<String>,
        /// List all available templates
        #[arg(long)]
        list: bool,
        /// The template name (positional)
        #[arg(required = false)]
        template: Option<String>,
        /// Enables verbose output
        #[arg(long)]
        verbose: bool,
        /// Execute the template but don't write to disk (hidden)
        #[arg(long, hide = true)]
        dry_run: bool,
    },
    /// Device screen operations
    Screen {
        #[command(subcommand)]
        command: ScreenCommands,
    },
    /// UI hierarchy dump
    Layout {
        /// Output file
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Show diff from last dump
        #[arg(long, short = 'd')]
        diff: bool,
        /// Pretty print JSON
        #[arg(long, short = 'p')]
        pretty: bool,
        /// Device serial
        #[arg(long)]
        device: Option<String>,
    },
    /// Shows the help of all commands
    Help {
        /// The command to show help for
        command: Option<String>,
    },
    /// Uploads metrics immediately (hidden)
    #[command(hide = true)]
    UploadMetrics,
    /// Hidden command for testing
    #[command(hide = true)]
    TestMetrics {
        #[command(subcommand)]
        command: Option<TestMetricsCommands>,
    },
}

#[derive(Subcommand)]
enum TestMetricsCommands {
    /// Crash the CLI for testing
    #[command(hide = true)]
    Crash {
        /// Crash from another thread
        #[arg(long)]
        thread: bool,
    },
    /// Report an invocation to metrics
    #[command(hide = true)]
    Report {
        /// A flag for the report subcommand
        #[arg(long, hide = true)]
        test_subcommand_flag: bool,
    },
}

#[derive(Subcommand)]
enum SdkCommands {
    /// Install SDK packages
    Install {
        /// Package specifications (e.g., "build-tools;34.0.0")
        packages: Vec<String>,

        /// Include canary packages
        #[arg(long)]
        canary: bool,

        /// Include beta packages
        #[arg(long)]
        beta: bool,

        /// Force installation (downgrade allowed)
        #[arg(long)]
        force: bool,
    },
    /// List SDK packages
    List {
        /// Show all available packages
        #[arg(long)]
        all: bool,

        /// Show all versions
        #[arg(long)]
        all_versions: bool,

        /// Filter packages by pattern (supports *)
        #[arg(required = false)]
        pattern: Option<String>,

        /// Include canary packages
        #[arg(long)]
        canary: bool,

        /// Include beta packages
        #[arg(long)]
        beta: bool,
    },
    /// Update SDK packages
    Update {
        /// Package to update (omit for all)
        package: Option<String>,

        /// Include canary packages
        #[arg(long)]
        canary: bool,

        /// Include beta packages
        #[arg(long)]
        beta: bool,

        /// Force update
        #[arg(long)]
        force: bool,
    },
    /// Remove SDK packages
    Remove {
        package: String,
    },
    /// Check SDK status
    #[command(hide = true)]
    Status,
    /// Download Android SDK from GitHub releases (for ARM/musl builds)
    /// Source: https://github.com/HomuHomu833/android-sdk-custom
    #[command(name = "arm")]
    Arm {
        /// SDK version to download (e.g., "36.0.2", "35.0.2"). Default: latest
        #[arg(long)]
        version: Option<String>,

        /// Target architecture. Default: auto-detect
        /// Options: aarch64, x86_64, x86, armhf, arm, riscv64, loongarch64, powerpc64le, s390x
        #[arg(long)]
        arch: Option<String>,

        /// List available versions and architectures
        #[arg(long)]
        list: bool,

        /// Number of parallel download threads. Default: 4
        #[arg(long, default_value = "4")]
        threads: usize,
    },

    // Hidden/internal commands for advanced operations
    /// Fetch SDK index from repository
    #[command(hide = true)]
    Fetch {
        /// Check for duplicate packages
        #[arg(long)]
        check: bool,
    },
    /// Resolve package id and version to SHA
    #[command(hide = true)]
    Resolve {
        /// Package path (e.g., "build-tools;34.0.0")
        packages: Vec<String>,

        /// Include canary packages
        #[arg(long)]
        canary: bool,

        /// Include beta packages
        #[arg(long)]
        beta: bool,
    },
    /// Materialize SDK units from SHA
    #[command(hide = true)]
    Materialize {
        /// SHA of SDK index to materialize
        sha: String,
    },
    /// Download packages by SHA
    #[command(hide = true)]
    Download {
        /// SHA of archive to download
        sha: String,
        /// URL to download from (optional, will use repository if not provided)
        #[arg(long)]
        url: Option<String>,
    },
    /// Print storage object by SHA
    #[command(hide = true)]
    Show {
        /// SHA of object to show
        sha: String,
        /// Show as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print SHA of a reference
    #[command(hide = true)]
    ShowRef {
        /// Reference name (e.g., "head", "remote")
        ref_name: String,
    },
    /// Commit current SDK to storage (returns SHA)
    #[command(hide = true)]
    Commit,
    /// Checkout package index and update SDK
    #[command(hide = true)]
    Checkout {
        /// SHA of SDK index to checkout
        sha: String,
        /// Force checkout even if local changes exist
        #[arg(long, short = 'f')]
        force: bool,
    },
    /// Install packages from index into another
    #[command(hide = true)]
    UpdateIndex {
        /// Source index SHA
        source_sha: String,
        /// Target index SHA (will be updated)
        target_sha: String,
    },
    /// Diff two SDK indexes (returns three SHAs)
    #[command(hide = true)]
    Diff {
        /// First SDK index SHA
        sha1: String,
        /// Second SDK index SHA
        sha2: String,
        /// Show detailed output
        #[arg(long)]
        verbose: bool,
    },
    /// Remove from disk by SHA
    #[command(hide = true)]
    Rm {
        /// SHA of object to remove
        sha: String,
        /// Remove archive as well
        #[arg(long)]
        archive: bool,
        /// Remove unzipped directory as well
        #[arg(long)]
        unzipped: bool,
    },
    /// Unzip downloaded packages in place
    #[command(hide = true)]
    Unzip {
        /// SHA of archive to unzip
        sha: String,
    },
    /// Generate legacy package.xml files
    #[command(hide = true)]
    WriteXml {
        /// Package path (e.g., "build-tools;34.0.0")
        package: String,
        /// SHA of the package archive
        #[arg(long)]
        sha: Option<String>,
    },
    /// Delete package from index
    #[command(hide = true)]
    DeleteIndex {
        /// SHA of SDK index
        sha: String,
        /// Package path to delete
        #[arg(long)]
        package: String,
    },
    /// Garbage collect storage
    #[command(hide = true)]
    Gc {
        /// Show what would be removed without actually removing
        #[arg(long)]
        dry_run: bool,
        /// Aggressive GC - remove all unreferenced objects
        #[arg(long)]
        aggressive: bool,
    },
}

#[derive(Subcommand)]
enum EmulatorCommands {
    /// Lists available virtual devices
    List {
        /// Give more detailed information
        #[arg(long)]
        long: bool,
    },
    /// Creates a virtual device
    Create {
        /// Lists the device profiles that can be used to create a device
        #[arg(long)]
        list_profiles: bool,
        /// Create a device with a specified profile
        #[arg(long, default_value = "pixel_6")]
        profile: String,
    },
    /// Launches the specified virtual device. This command will return when the emulator is fully started and ready to use.
    Start {
        /// The device (avd) to start. Use "android emulator list" to see available devices.
        #[arg(required = true)]
        device: String,
        /// Starts the emulator without loading from a snapshot
        #[arg(long)]
        cold: bool,
    },
    /// Stops the specified virtual device
    Stop {
        /// The emulator name or serial number to stop. Optional if only one emulator is running.
        #[arg(required = false)]
        device: Option<String>,
    },
    /// Delete a virtual device
    Remove {
        /// The device (avd) to remove. Use "android emulator list" to see available devices.
        #[arg(required = true)]
        device: String,
        /// Forces removal of .ini file even if the corresponding .avd directory doesn't exist
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum DeviceCommands {
    /// List connected devices
    List,
    /// Execute shell command
    Shell {
        device: String,
        command: Vec<String>,
    },
    /// Install APK
    Install {
        device: String,
        apks: Vec<String>,
    },
    /// Uninstall package
    Uninstall {
        device: String,
        package: String,
    },
    /// Port forwarding
    Forward {
        device: String,
        local: u16,
        remote: u16,
    },
}

#[derive(Subcommand)]
enum SkillsCommands {
    /// List available skills
    List {
        /// Use long output format
        #[arg(long)]
        long: bool,
        /// Path to a project root for which to list installed skills
        #[arg(long)]
        project: Option<String>,
    },
    /// Install a skill
    Add {
        /// The name of the skill to install
        #[arg(long)]
        skill: Option<String>,
        /// Install all skills
        #[arg(long)]
        all: bool,
        /// Comma-separated list of agents to install the skill for
        #[arg(long)]
        agent: Option<String>,
        /// Path to a project root in which to install
        #[arg(long)]
        project: Option<String>,
    },
    /// Remove a skill
    Remove {
        /// The name of the skill to remove
        #[arg(long)]
        skill: String,
        /// Comma-separated list of agents to remove the skill from
        #[arg(long)]
        agent: Option<String>,
        /// Path to a project root from which to remove
        #[arg(long)]
        project: Option<String>,
    },
    /// Find skills by keyword
    Find {
        /// Keyword to search for
        keyword: String,
    },
}

#[derive(Subcommand)]
enum DocsCommands {
    /// Search Android documentation
    Search {
        /// Search query
        query: String,
    },
    /// Fetch documentation content from URL
    Fetch {
        /// URL to fetch
        url: String,
    },
}

#[derive(Subcommand)]
enum ScreenCommands {
    /// Capture screenshot from device (Outputs the device screen to a PNG)
    Capture {
        /// Writes the screenshot to the specified file or directory
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Draws labeled bounding boxes around UI elements
        #[arg(long, short = 'a')]
        annotate: bool,
    },
    /// Resolve annotated screenshot coordinates
    /// Substitutes bounding box coordinates from a annotated screenshot into a string.
    /// Replaces all instances of '#N' with the center coordinates of the bounding box labeled 'N'
    Resolve {
        /// A screenshot captured with 'screen capture --annotate'
        #[arg(long)]
        screenshot: String,
        /// The string to substitute coordinates into
        #[arg(long)]
        string: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();

    let ctx = Context::new(
        cli.sdk,
        cli.sdk_index,
        cli.sdk_url,
        cli.no_metrics,
    )?;

    match cli.command {
        Commands::Sdk { command } => execute_sdk(command, &ctx),
        Commands::Emulator { command } => execute_emulator(command, &ctx),
        Commands::Device { command } => execute_device(command, &ctx),
        Commands::Run { apks, device, type_, activity, debug } => execute_run(&apks, &device, &type_, &activity, debug, &ctx),
        Commands::Skills { command } => execute_skills(command, &ctx),
        Commands::Template { process, profile, output } => execute_template(&process, &profile, &output, &ctx),
        Commands::Init { force } => execute_init(force, &ctx),
        Commands::Describe { project_dir } => execute_describe(project_dir.as_deref(), &ctx),
        Commands::Docs { command } => execute_docs(command),
        Commands::Update { url } => execute_update(url.as_deref()),
        Commands::Info { field } => execute_info(field.as_deref(), &ctx),
        Commands::Create { name, output, min_sdk, list, template, verbose, dry_run: _ } => execute_create(&name, &output, min_sdk.as_deref(), list, template.as_deref(), verbose, &ctx),
        Commands::Screen { command } => execute_screen(command, &ctx),
        Commands::Layout { output, diff, pretty, device } => execute_layout(output.as_deref(), diff, pretty, device.as_deref(), &ctx),
        Commands::Help { command } => execute_help(command.as_deref()),
        Commands::UploadMetrics => execute_upload_metrics(&ctx),
        Commands::TestMetrics { command } => execute_test_metrics(command),
    }?;

    Ok(())
}

struct Context {
    sdk_path: PathBuf,
    sdk_index: String,
    sdk_url: String,
    sys_info: SysInfoService,
    no_metrics: bool,
}

impl Context {
    fn new(sdk_path: Option<String>, sdk_index: String, sdk_url: String, no_metrics: bool) -> Result<Self> {
        let sys_info = SysInfoService::detect();
        let sdk_path = sdk_path
            .map(PathBuf::from)
            .or_else(|| std::env::var("ANDROID_HOME").ok().map(PathBuf::from))
            .unwrap_or_else(|| sys_info.default_sdk_path());

        Ok(Self { sdk_path, sdk_index, sdk_url, sys_info, no_metrics })
    }
}

struct SysInfoService {
    platform: Platform,
    arch: Architecture,
    user_home: PathBuf,
    android_user_home: PathBuf,
}

impl SysInfoService {
    fn detect() -> Self {
        let platform = match std::env::consts::OS {
            "macos" => Platform::Mac,
            "windows" => Platform::Windows,
            _ => Platform::Linux,
        };
        let arch = match std::env::consts::ARCH {
            "x86" | "i686" => Architecture::X86,
            "x86_64" | "amd64" => Architecture::X64,
            "aarch64" | "arm64" => Architecture::Aarch64,
            _ => Architecture::X64,
        };
        let user_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            platform,
            arch,
            user_home: user_home.clone(),
            android_user_home: user_home.join(".android"),
        }
    }

    fn default_sdk_path(&self) -> PathBuf {
        match self.platform {
            Platform::Mac => self.user_home.join("Library/Android/sdk"),
            Platform::Linux => self.user_home.join("Android/Sdk"),
            Platform::Windows => std::env::var("LOCALAPPDATA")
                .map(|p| PathBuf::from(p).join("Android/Sdk"))
                .ok()
                .unwrap_or_else(|| self.user_home.join("Android/Sdk")),
        }
    }

    fn cli_storage_path(&self) -> PathBuf { self.android_user_home.join("cli") }
}

#[derive(Debug, Clone, Copy)]
enum Platform { Linux, Mac, Windows }

#[derive(Debug, Clone, Copy)]
enum Architecture { X86, X64, Aarch64 }

/// Get channel from canary/beta flags (matches Kotlin getChannel)
fn get_channel_from_flags(canary: bool, beta: bool) -> Channel {
    if canary && beta {
        panic!("Error: --canary and --beta flags cannot be set at the same time.");
    }
    if canary {
        Channel::Canary
    } else if beta {
        Channel::Beta
    } else {
        Channel::Stable
    }
}

// SDK commands
fn execute_sdk(cmd: SdkCommands, ctx: &Context) -> Result<()> {
    let storage_path = ctx.sys_info.cli_storage_path();
    let manager = SdkManager::new(storage_path, ctx.sdk_path.clone(), &ctx.sdk_index, &ctx.sdk_url)?;

    match cmd {
        SdkCommands::Install { packages, canary, beta, force } => {
            let channel = get_channel_from_flags(canary, beta);
            manager.install(&packages, channel, force)?;
        }
        SdkCommands::List { all, all_versions, pattern, canary, beta } => {
            let channel = get_channel_from_flags(canary, beta);
            manager.list(all, all_versions, pattern.as_deref(), channel)?;
        }
        SdkCommands::Update { package, canary, beta, force } => {
            let channel = get_channel_from_flags(canary, beta);
            let packages = package.as_ref().map(|p| vec![p.clone()]);
            manager.update(packages.as_deref(), channel, force)?;
        }
        SdkCommands::Remove { package } => {
            manager.remove(&[package])?;
        }
        SdkCommands::Status => {
            manager.status(Channel::Stable)?;
        }
        // ARM/custom SDK download from GitHub releases
        SdkCommands::Arm { version, arch, list, threads } => {
            execute_sdk_arm(version.as_deref(), arch.as_deref(), list, threads, &ctx)?;
        }
        // Hidden commands
        SdkCommands::Fetch { check } => {
            manager.fetch(check)?;
        }
        SdkCommands::Resolve { packages, canary, beta } => {
            let channel = get_channel_from_flags(canary, beta);
            for pkg in &packages {
                manager.resolve(pkg, channel)?;
            }
        }
        SdkCommands::Materialize { sha } => {
            manager.materialize(&sha)?;
        }
        SdkCommands::Download { sha, url } => {
            manager.download(&sha, url.as_deref())?;
        }
        SdkCommands::Show { sha, json } => {
            manager.show(&sha, json)?;
        }
        SdkCommands::ShowRef { ref_name } => {
            manager.show_ref(&ref_name)?;
        }
        SdkCommands::Commit => {
            manager.commit_cmd()?;
        }
        SdkCommands::Checkout { sha, force } => {
            manager.checkout(&sha, force)?;
        }
        SdkCommands::UpdateIndex { source_sha, target_sha } => {
            manager.update_index(&source_sha, &target_sha)?;
        }
        SdkCommands::Diff { sha1, sha2, verbose } => {
            manager.diff_cmd(&sha1, &sha2, verbose)?;
        }
        SdkCommands::Rm { sha, archive, unzipped } => {
            manager.rm(&sha, archive, unzipped)?;
        }
        SdkCommands::Unzip { sha } => {
            manager.unzip_cmd(&sha)?;
        }
        SdkCommands::WriteXml { package, sha } => {
            manager.write_xml_cmd(&package, sha.as_deref())?;
        }
        SdkCommands::DeleteIndex { sha, package } => {
            manager.delete_index(&sha, &package)?;
        }
        SdkCommands::Gc { dry_run, aggressive } => {
            manager.gc_cmd(dry_run, aggressive)?;
        }
    }
    Ok(())
}

// ARM/custom SDK download command
fn execute_sdk_arm(version: Option<&str>, arch: Option<&str>, list: bool, threads: usize, ctx: &Context) -> Result<()> {
    use android_cli::sdk::{CustomSdkDownloader, CustomArch};

    let downloader = CustomSdkDownloader::with_threads(threads)?;

    if list {
        // List available versions
        downloader.list_versions()?;
        return Ok(());
    }

    // Parse architecture if provided
    let custom_arch = if let Some(arch_str) = arch {
        Some(parse_custom_arch(arch_str)?)
    } else {
        CustomArch::current()
    };

    // Install SDK
    downloader.install(version, custom_arch, &ctx.sdk_path)?;

    Ok(())
}

/// Parse architecture string to CustomArch
fn parse_custom_arch(s: &str) -> Result<android_cli::sdk::CustomArch> {
    use android_cli::sdk::CustomArch;

    match s.to_lowercase().as_str() {
        "aarch64" | "arm64" => Ok(CustomArch::Aarch64),
        "aarch64_be" => Ok(CustomArch::Aarch64Be),
        "armhf" | "arm" => Ok(CustomArch::Armhf),
        "arm-linux-musleabi" => Ok(CustomArch::Arm),
        "armeb" => Ok(CustomArch::Armeb),
        "armebhf" => Ok(CustomArch::ArmebHf),
        "x86" | "i686" => Ok(CustomArch::X86),
        "x86_64" | "amd64" => Ok(CustomArch::X86_64),
        "riscv32" => Ok(CustomArch::Riscv32),
        "riscv64" => Ok(CustomArch::Riscv64),
        "loongarch64" => Ok(CustomArch::Loongarch64),
        "powerpc64le" | "ppc64le" => Ok(CustomArch::Powerpc64le),
        "s390x" => Ok(CustomArch::S390x),
        _ => Err(anyhow::anyhow!(
            "Unknown architecture: {}. Valid options: aarch64, x86_64, x86, armhf, arm, riscv64, loongarch64, powerpc64le, s390x",
            s
        )),
    }
}

// Emulator commands
fn execute_emulator(cmd: EmulatorCommands, ctx: &Context) -> Result<()> {
    let avd_manager = AvdManager::new(ctx.sdk_path.clone())?;

    match cmd {
        EmulatorCommands::List { long } => {
            let avds = avd_manager.list()?;
            if avds.is_empty() {
                println!("No emulators found.");
            } else {
                for avd in avds {
                    if long {
                        println!("{}: {} [{}]", avd.name, avd.display_info(), if avd.running { "RUNNING" } else { "STOPPED" });
                    } else {
                        println!("{}", avd.name);
                    }
                }
            }
        }
        EmulatorCommands::Create { list_profiles, profile } => {
            if list_profiles {
                // Lists the device profiles that can be used to create a device
                println!("Available device profiles:");
                for (name, desc) in AvdManager::list_profiles() {
                    println!("  {} - {}", name, desc);
                }
                return Ok(());
            }
            // Create a device with specified profile
            // Kotlin version auto-generates name and uses default API
            // We'll use the profile as a hint for name and API
            let default_api = 34;
            let name = format!("{}_api{}", profile.replace("_", "-"), default_api);
            avd_manager.create(&name, &profile, default_api)?;
        }
        EmulatorCommands::Start { device, cold } => {
            // Launches the specified virtual device
            println!("Starting emulator {}...", device);
            avd_manager.start(&device, cold)?;
        }
        EmulatorCommands::Stop { device } => {
            // Stops the specified virtual device
            avd_manager.stop(device.as_deref())?;
        }
        EmulatorCommands::Remove { device, force } => {
            // Delete a virtual device
            avd_manager.remove(&device, force)?;
        }
    }
    Ok(())
}

// Device commands
fn execute_device(cmd: DeviceCommands, ctx: &Context) -> Result<()> {
    let adb = AdbService::new(&ctx.sdk_path)?;

    match cmd {
        DeviceCommands::List => {
            let devices = adb.devices()?;
            if devices.is_empty() {
                println!("No devices connected.");
            } else {
                for device in devices {
                    println!("{} {} {}", device.serial, device.state, device.model.as_deref().unwrap_or(""));
                }
            }
        }
        DeviceCommands::Shell { device, command } => {
            let cmd_str = command.join(" ");
            let output = adb.shell(&device, &cmd_str)?;
            println!("{}", output);
        }
        DeviceCommands::Install { device, apks } => {
            if apks.len() == 1 {
                adb.install(&device, &PathBuf::from(&apks[0]))?;
            } else {
                let paths: Vec<PathBuf> = apks.iter().map(PathBuf::from).collect();
                adb.install_multiple(&device, &paths)?;
            }
        }
        DeviceCommands::Uninstall { device, package } => {
            adb.uninstall(&device, &package)?;
        }
        DeviceCommands::Forward { device, local, remote } => {
            adb.forward(&device, &format!("tcp:{}", local), &format!("tcp:{}", remote))?;
        }
    }
    Ok(())
}

// Run command
fn execute_run(apks: &[String], device: &Option<String>, type_: &str, activity: &Option<String>, debug: bool, ctx: &Context) -> Result<()> {
    let adb = AdbService::new(&ctx.sdk_path)?;

    // Select device
    let target_device = if let Some(d) = device {
        d.clone()
    } else {
        let devices = adb.devices()?;
        let first = devices.first()
            .ok_or_else(|| anyhow::anyhow!("No devices connected"))?;
        first.serial.clone()
    };

    // Install APKs
    let paths: Vec<PathBuf> = apks.iter().map(PathBuf::from).collect();
    if paths.len() == 1 {
        adb.install(&target_device, &paths[0])?;
    } else {
        adb.install_multiple(&target_device, &paths)?;
    }

    // Get package name from APK
    let package = get_package_from_apk(&paths[0])?;

    // Launch activity
    if type_ == "activity" {
        let activity_name = activity.clone()
            .unwrap_or_else(|| format!("{}.MainActivity", package));
        adb.launch_activity(&target_device, &package, &activity_name)?;
    }

    if debug {
        println!("Debug mode enabled. Connect debugger to {}", target_device);
    }

    Ok(())
}

fn get_package_from_apk(apk_path: &PathBuf) -> Result<String> {
    // Use aapt to get package name (or parse APK manifest)
    let output = std::process::Command::new("aapt")
        .arg("dump").arg("badging").arg(apk_path)
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if line.starts_with("package: name=") {
                    let parts = line.split_whitespace().collect::<Vec<_>>();
                    for part in parts {
                        if part.starts_with("name=") {
                            return Ok(part.split('=').nth(1).unwrap_or("unknown").to_string());
                        }
                    }
                }
            }
            Err(anyhow::anyhow!("Could not parse package name from APK"))
        }
        Err(_) => {
            // Fallback: use filename as package hint
            let name = apk_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            println!("Warning: aapt not found, using '{}' as package hint", name);
            Ok(name.replace(".apk", ""))
        }
    }
}

// Skills commands
fn execute_skills(cmd: SkillsCommands, ctx: &Context) -> Result<()> {
    let manager = SkillManager::new()?;
    let _ = ctx; // unused

    match cmd {
        SkillsCommands::List { long, project } => {
            let project_path = project.as_ref().map(PathBuf::from);
            let skills = manager.list(None, project_path.as_ref())?;
            if skills.is_empty() {
                println!("No skills installed.");
            } else {
                for skill in skills {
                    if long {
                        println!("{}: {} (v{})", skill.name, skill.description, skill.version);
                        if skill.has_claude { println!("  - CLAUDE.md"); }
                        if skill.has_gemini { println!("  - GEMINI.md"); }
                    } else {
                        println!("{}", skill.name);
                    }
                }
            }
        }
        SkillsCommands::Add { skill, all, agent, project } => {
            let project_path = project.as_ref().map(PathBuf::from);
            manager.add(skill.as_deref(), all, agent.as_deref(), project_path.as_ref())?;
        }
        SkillsCommands::Remove { skill, agent, project } => {
            let project_path = project.as_ref().map(PathBuf::from);
            manager.remove(&skill, agent.as_deref(), project_path.as_ref())?;
        }
        SkillsCommands::Find { keyword } => {
            let results = manager.find(&keyword)?;
            if results.is_empty() {
                println!("No skills found matching '{}'", keyword);
            } else {
                for skill in results {
                    println!("{}: {}", skill.name, skill.description);
                }
            }
        }
    }
    Ok(())
}

// Template commands
fn execute_template(process: &Option<String>, profile: &Option<String>, output: &Option<String>, ctx: &Context) -> Result<()> {
    let _ = ctx; // unused

    if process.is_none() {
        println!("Available device templates:");
        for (name, desc) in DeviceTemplates::list() {
            println!("  {} - {}", name, desc);
        }
        return Ok(());
    }

    let template_path = PathBuf::from(process.as_ref().unwrap());
    let output_path = output.as_ref().map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut processor = TemplateProcessor::new();

    // Add profile variables if specified
    if let Some(p) = profile {
        let config = DeviceTemplates::get_config(p)
            .ok_or_else(|| anyhow::anyhow!("Unknown profile: {}", p))?;
        processor.set_vars(config);
    }

    if template_path.is_dir() {
        processor.process_dir(&template_path, &output_path)?;
    } else {
        processor.process_file(&template_path, &output_path)?;
    }

    Ok(())
}

// Init command
fn execute_init(force: bool, ctx: &Context) -> Result<()> {
    println!("Initializing Android CLI...");

    // Create storage directory
    let storage_path = ctx.sys_info.cli_storage_path();
    std::fs::create_dir_all(&storage_path)?;
    println!("Created storage: {}", storage_path.display());

    // Create skills directories
    let home = ctx.sys_info.user_home.clone();
    let skills_dir = home.join(".claude").join("skills");
    std::fs::create_dir_all(&skills_dir)?;
    println!("Created skills directory: {}", skills_dir.display());

    // Install bundled android-cli skill
    let bundled_skill_source = skills_dir.join("android-cli");
    let skill_already_installed = bundled_skill_source.exists() && bundled_skill_source.join("SKILL.md").exists();

    if !skill_already_installed || force {
        std::fs::create_dir_all(&bundled_skill_source)?;

        // Create SKILL.md with Android CLI development instructions
        let skill_md_content = r#"---
name: android-cli
description: Android CLI development skill for AI agents
version: "1.0"
author: android-cli
tags:
  - android
  - cli
  - development
  - sdk
  - emulator
---

# Android CLI Development Skill

This skill provides instructions for AI agents working with Android development using the `android` CLI tool.

## Core Commands

### SDK Management
- `android sdk install <package>` - Install SDK packages (e.g., "build-tools;34.0.0", "platforms;34")
- `android sdk list --all` - List all available packages
- `android sdk status` - Check SDK installation status

### Emulator Management
- `android emulator list` - List available AVDs
- `android emulator create --name <name> --api <level> --profile <profile>` - Create new emulator
- `android emulator start <device>` - Start an emulator
- `android emulator stop <device>` - Stop an emulator

### Device Management
- `android device list` - List connected devices
- `android device shell <device> <command>` - Execute shell command on device
- `android device install <device> <apk>` - Install APK on device

### Project Analysis
- `android describe --project-dir <path>` - Analyze Android project structure
- Output includes: modules, build variants, SDK versions, APK locations

### Running Apps
- `android run -a <apk>` - Install and run APK on connected device
- `android run -a <apk> -d <device>` - Run on specific device

## Common Workflows

### Setting Up Development Environment
```
android init
android sdk install "build-tools;34.0.0" "platforms;34" "platform-tools"
```

### Creating and Running Emulator
```
android emulator create --name test_emu --api 34 --profile pixel_6
android emulator start test_emu
android device list
```

### Installing and Running App
```
android run -a app/build/outputs/apk/debug/app-debug.apk
```

## Project Structure Analysis

When analyzing an Android project, `android describe --project-dir` provides:
- Gradle version detection
- AGP and Kotlin versions
- Module discovery (application, library, dynamic-feature)
- Build variants (productFlavors + buildTypes)
- Min/target SDK versions
- Expected APK output locations

## Best Practices

1. Always check SDK status before installing packages
2. Use specific package versions for reproducible builds
3. Verify device/emulator availability before running apps
4. Analyze project structure before making build changes
"#;
        std::fs::write(bundled_skill_source.join("SKILL.md"), skill_md_content)?;

        if skill_already_installed && force {
            println!("Reinstalled bundled skill: android-cli");
        } else {
            println!("Installed bundled skill: android-cli");
        }
    } else {
        println!("Bundled skill android-cli already installed (use --force to reinstall)");
    }

    // Verify SDK path
    println!("SDK path: {}", ctx.sdk_path.display());
    if ctx.sdk_path.exists() {
        println!("SDK found.");
    } else {
        println!("Warning: SDK path does not exist. Install SDK packages first.");
    }

    println!("Initialization complete.");
    Ok(())
}

// Describe command
fn execute_describe(project_dir: Option<&str>, ctx: &Context) -> Result<()> {
    let describe_cli = DescribeCLI::new(Some(ctx.sdk_path.clone()));

    if let Some(dir) = project_dir {
        // Analyze Android project structure
        let project_path = PathBuf::from(dir);
        let description = describe_cli.analyze_project(&project_path)?;

        // Output as JSON
        let json = serde_json::to_string_pretty(&description)?;
        println!("{}", json);
    } else {
        // Describe SDK packages (default behavior)
        describe_cli.describe_sdk(&ctx.sdk_path)?;
    }

    Ok(())
}

// Docs command
fn execute_docs(command: Option<DocsCommands>) -> Result<()> {
    match command {
        Some(DocsCommands::Search { query }) => {
            let docs_cli = DocsCLI::new()?;
            let results = docs_cli.search(&query)?;
            DocsCLI::display_search_results(&results);
        }
        Some(DocsCommands::Fetch { url }) => {
            let docs_cli = DocsCLI::new()?;
            let content = docs_cli.fetch(&url)?;
            println!("{}", content);
        }
        None => {
            // Default behavior when no subcommand provided
            println!("Android CLI Documentation:");
            println!("  SDK Manager: https://developer.android.com/tools/sdkmanager");
            println!("  Emulator:    https://developer.android.com/tools/emulator");
            println!("  ADB:         https://developer.android.com/tools/adb");
            println!("  AVD Manager: https://developer.android.com/tools/avdmanager");
            println!();
            println!("Commands:");
            println!("  android docs search <query>  - Search Android documentation");
            println!("  android docs fetch <url>     - Fetch documentation from URL");
            println!("  android sdk install <package>  - Install SDK package");
            println!("  android sdk list --all         - List available packages");
            println!("  android emulator list          - List AVDs");
            println!("  android device list            - List connected devices");
            println!("  android run -a <apk>           - Install and run APK");
        }
    }
    Ok(())
}

// Update command
fn execute_update(url: Option<&str>) -> Result<()> {
    let updater = if let Some(custom_url) = url {
        Updater::with_url(custom_url)
    } else {
        Updater::new()
    };

    updater.update(url)?;

    Ok(())
}

// Info command
fn execute_info(field: Option<&str>, ctx: &Context) -> Result<()> {
    // Get CLI version
    let version = env!("CARGO_PKG_VERSION");

    // Get platform string
    let platform_str = match ctx.sys_info.platform {
        Platform::Mac => "macos",
        Platform::Linux => "linux",
        Platform::Windows => "windows",
    };

    // Get architecture
    let arch_str = match std::env::consts::ARCH {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "aarch64",
        other => other,
    };

    match field {
        Some(f) => {
            // Print specific field
            let value = match f {
                "sdk-path" => ctx.sdk_path.display().to_string(),
                "user-home" => ctx.sys_info.user_home.display().to_string(),
                "android-home" => ctx.sys_info.android_user_home.display().to_string(),
                "platform" => platform_str.to_string(),
                "arch" => arch_str.to_string(),
                "version" => version.to_string(),
                other => {
                    return Err(anyhow::anyhow!(
                        "Unknown field: {}. Valid fields: sdk-path, user-home, android-home, platform, arch, version",
                        other
                    ));
                }
            };
            println!("{}", value);
        }
        None => {
            // Print all fields
            println!("Android CLI Environment Information:");
            println!("  SDK Path:      {}", ctx.sdk_path.display());
            println!("  User Home:     {}", ctx.sys_info.user_home.display());
            println!("  Android Home:  {}", ctx.sys_info.android_user_home.display());
            println!("  Platform:      {}", platform_str);
            println!("  Architecture:  {}", arch_str);
            println!("  CLI Version:   {}", version);
        }
    }

    Ok(())
}

// Create command
fn execute_create(
    name: &str,
    output: &str,
    min_sdk: Option<&str>,
    list: bool,
    template: Option<&str>,
    verbose: bool,
    ctx: &Context,
) -> Result<()> {
    let _ = ctx; // unused

    let runner = TemplateEngineRunner::new();

    if list {
        runner.print_templates()?;
        return Ok(());
    }

    let template_name = template.ok_or_else(|| anyhow::anyhow!(
        "Template name required. Use: android create <template> --name <project-name>"
    ))?;

    let output_path = PathBuf::from(output);
    runner.create_project(template_name, name, &output_path, min_sdk, verbose)?;

    Ok(())
}

// Screen command
fn execute_screen(command: ScreenCommands, ctx: &Context) -> Result<()> {
    // Get device from context or auto-select (matches Kotlin behavior)
    let screen_cmd = ScreenCommand::new(&ctx.sdk_path)?;

    match command {
        ScreenCommands::Capture { output, annotate } => {
            // Kotlin version uses AdbKt.getDevice which auto-selects if no device specified
            // We pass None for device to match Kotlin behavior
            screen_cmd.capture(None, output.as_deref(), annotate)?;
        }
        ScreenCommands::Resolve { screenshot, string } => {
            let result = ResolveCommand::resolve(&screenshot, &string)?;
            println!("{}", result);
        }
    }

    Ok(())
}

// Layout command
fn execute_layout(
    output: Option<&str>,
    diff: bool,
    pretty: bool,
    device: Option<&str>,
    ctx: &Context,
) -> Result<()> {
    let layout_cmd = LayoutCommand::new(&ctx.sdk_path)?;
    layout_cmd.dump(device, output, diff, pretty)?;

    Ok(())
}

// Help command
fn execute_help(command: Option<&str>) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");

    if let Some(cmd_name) = command {
        // Show help for specific command
        print_command_help(cmd_name);
    } else {
        // Show general help
        println!("Android CLI v{} - Android development tools CLI", version);
        println!();
        println!("Usage: android [OPTIONS] <COMMAND>");
        println!();
        println!("Commands:");

        // Visible commands
        let visible_commands = [
            ("sdk", "Download and list SDK packages"),
            ("emulator", "Emulator commands"),
            ("device", "Device management (requires ADB)"),
            ("run", "Run APK on device"),
            ("skills", "AI Agent Skills management"),
            ("template", "Template processing"),
            ("init", "Initialize Android CLI environment"),
            ("describe", "Describe SDK packages or analyze project"),
            ("docs", "Documentation search and fetch"),
            ("update", "Self-update the CLI"),
            ("info", "Print environment information"),
            ("create", "Create new Android project from template"),
            ("screen", "Device screen operations"),
            ("layout", "UI hierarchy dump"),
            ("help", "Show help for all commands"),
        ];

        for (name, desc) in visible_commands {
            println!("  {:12} {}", name, desc);
        }

        println!();
        println!("Global Options:");
        println!("  --sdk <PATH>       Path to Android SDK");
        println!("  --no-metrics       Disable metrics collection");
        println!("  --version          Show version");
        println!("  --help             Show help");

        println!();
        println!("Run 'android help <command>' for detailed help on a specific command.");
    }

    Ok(())
}

fn print_command_help(cmd_name: &str) {
    match cmd_name {
        "sdk" => {
            println!("Download and list SDK packages");
            println!();
            println!("Usage: android sdk <SUBCOMMAND>");
            println!();
            println!("Subcommands:");
            println!("  install <packages> [--canary] [--beta] [--force]  Install SDK packages");
            println!("  list [--all] [--all-versions] [--canary] [--beta]  List packages");
            println!("  update [package] [--canary] [--beta] [--force]    Update packages");
            println!("  remove <package>                                  Remove a package");
            println!("  arm [--version <v>] [--arch <arch>] [--threads <n>] [--list]  Download ARM/musl SDK");
            println!("                                                    Parallel download with N threads (default: 4)");
            println!("                                                    (https://github.com/HomuHomu833/android-sdk-custom)");
        }
        "emulator" => {
            println!("Emulator management");
            println!();
            println!("Usage: android emulator <SUBCOMMAND>");
            println!();
            println!("Subcommands:");
            println!("  list                        List available AVDs");
            println!("  create --name <n> --api <a> --profile <p>  Create new emulator");
            println!("  start <device> [--cold]     Start an emulator");
            println!("  stop [device]               Stop emulator(s)");
            println!("  remove <device> [--force]   Delete an AVD");
        }
        "device" => {
            println!("Device management (requires ADB) - Hidden command");
            println!();
            println!("Usage: android device <SUBCOMMAND>");
            println!();
            println!("Subcommands:");
            println!("  list                      List connected devices");
            println!("  shell <device> <cmd>      Execute shell command");
            println!("  install <device> <apk>    Install APK");
            println!("  uninstall <device> <pkg>  Uninstall package");
            println!("  forward <device> <l> <r>  Port forwarding");
        }
        "run" => {
            println!("Deploy an Android Application");
            println!();
            println!("Usage: android run --apks <apk> [--device <serial>] [--type <type>] [--activity <name>] [--debug]");
            println!();
            println!("Options:");
            println!("  --apks <apk>          The paths to the APKs (required)");
            println!("  --device <serial>     The device serial number");
            println!("  --type <type>         The component type (ACTIVITY, SERVICE, etc.)");
            println!("  --activity <name>     The activity name");
            println!("  --debug               Run in debug mode");
        }
        "screen" => {
            println!("Commands to view the device");
            println!();
            println!("Usage: android screen <SUBCOMMAND>");
            println!();
            println!("Subcommands:");
            println!("  capture [-o <file>] [-a]         Outputs the device screen to a PNG");
            println!("    -o, --output                   Writes the screenshot to the specified file or directory");
            println!("    -a, --annotate                 Draws labeled bounding boxes around UI elements");
            println!("  resolve --screenshot <file> --string <str>");
            println!("    --screenshot                   A screenshot captured with 'screen capture --annotate'");
            println!("    --string                       The string to substitute coordinates into");
        }
        "layout" => {
            println!("Returns the layout tree of an application");
            println!();
            println!("Usage: android layout [-o <file>] [-d] [-p] [--device <serial>]");
            println!();
            println!("Options:");
            println!("  -o, --output <file>  Writes the layout tree to the specified file");
            println!("  -d, --diff           Returns flat list of elements that changed since last dump");
            println!("  -p, --pretty         Pretty-prints the returned JSON");
            println!("  --device <serial>    The device serial number");
        }
        "create" => {
            println!("Create a new Android project");
            println!();
            println!("Usage: android create [template] --name <name> [-o <dest-path>] [--minSdk <version>] [--verbose]");
            println!();
            println!("Arguments:");
            println!("  <template>           The template name (positional)");
            println!();
            println!("Options:");
            println!("  --name <name>        The name of the application (required)");
            println!("  -o, --output <dir>   The destination project directory path (default: '.')");
            println!("  --minSdk <version>   The minSdk supported by the application");
            println!("  --list               List all available templates");
            println!("  --verbose            Enables verbose output");
        }
        _ => {
            println!("Unknown command: '{}'", cmd_name);
            println!("Run 'android help' to see available commands.");
        }
    }
}

// UploadMetrics command (hidden)
fn execute_upload_metrics(ctx: &Context) -> Result<()> {
    use android_cli::metrics::{MetricsConfig, MetricsUploader};

    println!("Uploading metrics...");

    let version = env!("CARGO_PKG_VERSION");
    let config = MetricsConfig::new(!ctx.no_metrics, &ctx.sys_info.android_user_home, version);

    let uploader = MetricsUploader::new(config);
    let result = uploader.upload_now()?;

    println!("  Invocations uploaded: {}", result.invocations_uploaded);
    println!("  Crashes uploaded: {}", result.crashes_uploaded);
    println!("  Status: {}", result.message);

    // Also upload crash reports
    uploader.upload_crash_reports()?;

    println!("Metrics upload complete.");

    Ok(())
}

// TestMetrics command (hidden)
fn execute_test_metrics(command: Option<TestMetricsCommands>) -> Result<()> {
    match command {
        Some(TestMetricsCommands::Crash { thread }) => {
            if thread {
                std::thread::spawn(|| {
                    panic!("Test crash from thread");
                }).join().unwrap();
            } else {
                panic!("Test crash");
            }
        }
        Some(TestMetricsCommands::Report { test_subcommand_flag: _ }) => {
            println!("Report invocation test (no metrics in Rust version)");
        }
        None => {
            println!("test-metrics: hidden command for testing");
        }
    }
    Ok(())
}