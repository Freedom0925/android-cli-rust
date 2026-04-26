use clap::{Parser, Subcommand};

use android_cli::config::{DEFAULT_SDK_INDEX_URL, DEFAULT_SDK_URL};

/// Android CLI - Pure Rust implementation
#[derive(Parser)]
#[command(name = "android")]
#[command(version = "0.1.1")]
#[command(about = "Android development tools CLI")]
#[command(
    long_about = "Android CLI provides tools for SDK management, emulator control, device interaction, and more."
)]
#[command(disable_help_subcommand = true)]
pub struct Cli {
    /// Path to Android SDK
    #[arg(long, global = true)]
    pub sdk: Option<String>,

    /// Disable metrics collection (hidden)
    #[arg(long, global = true, hide = true)]
    pub no_metrics: bool,

    /// SDK index URL (hidden)
    #[arg(
        long,
        global = true,
        hide = true,
        default_value = DEFAULT_SDK_INDEX_URL
    )]
    pub sdk_index: String,

    /// SDK artifact URL (hidden)
    #[arg(
        long,
        global = true,
        hide = true,
        default_value = DEFAULT_SDK_URL
    )]
    pub sdk_url: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
        /// The name of the application (e.g. 'My Application') - REQUIRED unless --list
        #[arg(long, required_unless_present = "list")]
        name: Option<String>,
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
pub enum TestMetricsCommands {
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
pub enum SdkCommands {
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
    Remove { package: String },
    /// Check SDK status
    #[command(hide = true)]
    Status,

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
pub enum EmulatorCommands {
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
pub enum DeviceCommands {
    /// List connected devices
    List,
    /// Execute shell command
    Shell {
        device: String,
        command: Vec<String>,
    },
    /// Install APK
    Install { device: String, apks: Vec<String> },
    /// Uninstall package
    Uninstall { device: String, package: String },
    /// Port forwarding
    Forward {
        device: String,
        local: u16,
        remote: u16,
    },
}

#[derive(Subcommand)]
pub enum SkillsCommands {
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
pub enum DocsCommands {
    /// Search Android documentation using KB local index
    Search {
        /// Search query
        query: String,
    },
    /// Show KB index statistics
    Stats,
    /// Clear KB cache (re-download on next search)
    Clear,
    /// [deprecated] Fetch is not available with KB-based search
    Fetch {
        /// URL to fetch
        url: String,
    },
}

#[derive(Subcommand)]
pub enum ScreenCommands {
    /// Capture screenshot from device (Outputs the device screen to a PNG)
    Capture {
        /// Writes the screenshot to the specified file or directory
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Draws labeled bounding boxes around UI elements
        #[arg(long, short = 'a')]
        annotate: bool,
        /// Cluster merge threshold for feature detection (default: 10)
        #[arg(long, default_value = "10")]
        cluster_merge_threshold: i32,
        /// Output intermediate debug images to debug/ directory
        #[arg(long)]
        debug: bool,
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
