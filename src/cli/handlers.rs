//! CLI command handlers
//!
//! Contains all execute_* functions for processing CLI commands

use anyhow::Result;
use std::path::PathBuf;

use android_cli::adb::AdbService;
use android_cli::create::TemplateEngineRunner;
use android_cli::describe::DescribeCLI;
use android_cli::docs::DocsCLI;
use android_cli::emulator::AvdManager;
use android_cli::layout::LayoutCommand;
use android_cli::screen::{ResolveCommand, ScreenCommand};
use android_cli::sdk::{Channel, SdkManager};
use android_cli::sdk::protobuf::Platform;
use android_cli::skills::SkillManager;
use android_cli::template::{DeviceTemplates, TemplateProcessor};
use android_cli::update::Updater;

use super::commands::*;
use super::context::{Context, get_channel_from_flags};

// SDK commands
pub fn execute_sdk(cmd: SdkCommands, ctx: &Context) -> Result<()> {
    let storage_path = ctx.sys_info.cli_storage_path();
    let manager = SdkManager::new(
        storage_path,
        ctx.sdk_path.clone(),
        &ctx.sdk_index,
        &ctx.sdk_url,
    )?;

    match cmd {
        SdkCommands::Install {
            packages,
            canary,
            beta,
            force,
        } => {
            let channel = get_channel_from_flags(canary, beta)?;
            manager.install(&packages, channel, force)?;
        }
        SdkCommands::List {
            all,
            all_versions,
            pattern,
            canary,
            beta,
        } => {
            let channel = get_channel_from_flags(canary, beta)?;
            manager.list(all, all_versions, pattern.as_deref(), channel)?;
        }
        SdkCommands::Update {
            package,
            canary,
            beta,
            force,
        } => {
            let channel = get_channel_from_flags(canary, beta)?;
            let packages = package.as_ref().map(|p| vec![p.clone()]);
            manager.update(packages.as_deref(), channel, force)?;
        }
        SdkCommands::Remove { package } => {
            manager.remove(&[package])?;
        }
        SdkCommands::Status => {
            manager.status(Channel::Stable)?;
        }
        // Hidden commands
        SdkCommands::Fetch { check } => {
            manager.fetch(check)?;
        }
        SdkCommands::Resolve {
            packages,
            canary,
            beta,
        } => {
            let channel = get_channel_from_flags(canary, beta)?;
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
        SdkCommands::UpdateIndex {
            source_sha,
            target_sha,
        } => {
            manager.update_index(&source_sha, &target_sha)?;
        }
        SdkCommands::Diff {
            sha1,
            sha2,
            verbose,
        } => {
            manager.diff_cmd(&sha1, &sha2, verbose)?;
        }
        SdkCommands::Rm {
            sha,
            archive,
            unzipped,
        } => {
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
        SdkCommands::Gc {
            dry_run,
            aggressive,
        } => {
            manager.gc_cmd(dry_run, aggressive)?;
        }
    }
    Ok(())
}

// Emulator commands
pub fn execute_emulator(cmd: EmulatorCommands, ctx: &Context) -> Result<()> {
    let avd_manager = AvdManager::new(ctx.sdk_path.clone())?;

    match cmd {
        EmulatorCommands::List { long } => {
            let avds = avd_manager.list()?;
            if avds.is_empty() {
                println!("No emulators found.");
            } else {
                for avd in avds {
                    if long {
                        println!(
                            "{}: {} [{}]",
                            avd.name,
                            avd.display_info(),
                            if avd.running { "RUNNING" } else { "STOPPED" }
                        );
                    } else {
                        println!("{}", avd.name);
                    }
                }
            }
        }
        EmulatorCommands::Create {
            list_profiles,
            profile,
        } => {
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
pub fn execute_device(cmd: DeviceCommands, ctx: &Context) -> Result<()> {
    let adb = AdbService::new(&ctx.sdk_path)?;

    match cmd {
        DeviceCommands::List => {
            let devices = adb.devices()?;
            if devices.is_empty() {
                println!("No devices connected.");
            } else {
                for device in devices {
                    println!(
                        "{} {} {}",
                        device.serial,
                        device.state,
                        device.model.as_deref().unwrap_or("")
                    );
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
        DeviceCommands::Forward {
            device,
            local,
            remote,
        } => {
            adb.forward(
                &device,
                &format!("tcp:{}", local),
                &format!("tcp:{}", remote),
            )?;
        }
    }
    Ok(())
}

// Run command
pub fn execute_run(
    apks: &[String],
    device: &Option<String>,
    type_: &str,
    activity: &Option<String>,
    debug: bool,
    ctx: &Context,
) -> Result<()> {
    let adb = AdbService::new(&ctx.sdk_path)?;

    // Select device
    let target_device = if let Some(d) = device {
        d.clone()
    } else {
        let devices = adb.devices()?;
        let first = devices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No devices connected"))?;
        first.serial.clone()
    };

    // Get package list before installation (for detecting new package)
    let packages_before = adb.list_packages(&target_device, None)?;

    // Install APKs
    let paths: Vec<PathBuf> = apks.iter().map(PathBuf::from).collect();
    println!(
        "Installing APKs: {}",
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if paths.len() == 1 {
        adb.install(&target_device, &paths[0])?;
    } else {
        adb.install_multiple(&target_device, &paths)?;
    }
    println!("Installation completed successfully");

    // Get package name - try multiple methods
    let package = get_package_from_apk_or_device(
        &paths[0],
        &adb,
        &target_device,
        &packages_before,
        &ctx.sdk_path,
    )?;

    println!("App loaded: {}", package);

    // Launch activity using monkey (more reliable)
    if type_ == "activity" {
        if let Some(act) = activity {
            // Use specified activity
            let full_activity = if act.starts_with('.') {
                format!("{}{}", package, act)
            } else {
                act.to_string()
            };
            println!("Selected component: {}", full_activity);
            adb.launch_activity(&target_device, &package, &full_activity)?;
        } else {
            // Use monkey to launch default activity
            println!("Launching {} (using monkey)", package);
            adb.monkey_launch(&target_device, &package)?;
        }
    }

    if debug {
        println!("Debug mode enabled. Connect debugger to {}", target_device);
    }

    Ok(())
}

/// Get package name from APK or detect from device after installation
pub fn get_package_from_apk_or_device(
    apk_path: &PathBuf,
    adb: &AdbService,
    serial: &str,
    packages_before: &[String],
    sdk_path: &PathBuf,
) -> Result<String> {
    // Find aapt in SDK build-tools (use highest version)
    let aapt_path = find_aapt_in_sdk(sdk_path);

    // First try aapt
    let output = if let Some(aapt) = aapt_path {
        std::process::Command::new(&aapt)
            .arg("dump")
            .arg("badging")
            .arg(apk_path)
            .output()
    } else {
        // Try system aapt as fallback
        std::process::Command::new("aapt")
            .arg("dump")
            .arg("badging")
            .arg(apk_path)
            .output()
    };

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if line.starts_with("package: name=") {
                    // Format: package: name='com.example.app' versionCode='1' versionName='1.0'
                    for part in line.split_whitespace() {
                        if part.starts_with("name=") {
                            // Remove quotes if present
                            let name = part.split('=').nth(1).unwrap_or("").replace("'", "");
                            if !name.is_empty() {
                                return Ok(name);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // aapt failed or not found - detect from device
    println!("Warning: aapt not found or failed, detecting package from device...");

    // Get packages after installation
    let packages_after = adb.list_packages(serial, None)?;

    // Find new package (installed since before)
    let new_packages: Vec<String> = packages_after
        .iter()
        .filter(|p| !packages_before.contains(p))
        .cloned()
        .collect();

    if new_packages.len() == 1 {
        println!("Detected new package: {}", new_packages[0]);
        return Ok(new_packages[0].clone());
    } else if new_packages.len() > 1 {
        // Multiple new packages - try to match by APK name
        let apk_name = apk_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        for pkg in &new_packages {
            // Try partial match on package name
            if pkg.contains(
                &apk_name
                    .replace("-v", "")
                    .replace(".apk", "")
                    .replace(" ", "")
                    .to_lowercase(),
            ) {
                println!("Matched package by name: {}", pkg);
                return Ok(pkg.clone());
            }
        }

        // Return first new package as fallback
        println!(
            "Multiple new packages detected, using first: {}",
            new_packages[0]
        );
        return Ok(new_packages[0].clone());
    }

    // No new package detected - this shouldn't happen after successful install
    Err(anyhow::anyhow!(
        "Could not determine package name after installation"
    ))
}

/// Find aapt in SDK build-tools directory (use highest version)
pub fn find_aapt_in_sdk(sdk_path: &PathBuf) -> Option<PathBuf> {
    let build_tools_dir = sdk_path.join("build-tools");
    if !build_tools_dir.exists() {
        return None;
    }

    // List all build-tools versions and pick the highest
    let mut versions: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&build_tools_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip non-version directories (like .DS_Store)
            if name.starts_with('.') {
                continue;
            }
            // Check if aapt exists in this directory
            if entry.path().join("aapt").exists() {
                versions.push(name);
            }
        }
    }

    // Sort versions (highest first) - simple string sort works for X.Y.Z format
    versions.sort();
    versions.reverse();

    if let Some(highest) = versions.first() {
        Some(build_tools_dir.join(highest).join("aapt"))
    } else {
        None
    }
}

// Skills commands
pub fn execute_skills(cmd: SkillsCommands, ctx: &Context) -> Result<()> {
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
                        if skill.has_claude {
                            println!("  - CLAUDE.md");
                        }
                        if skill.has_gemini {
                            println!("  - GEMINI.md");
                        }
                    } else {
                        println!("{}", skill.name);
                    }
                }
            }
        }
        SkillsCommands::Add {
            skill,
            all,
            agent,
            project,
        } => {
            let project_path = project.as_ref().map(PathBuf::from);
            manager.add(
                skill.as_deref(),
                all,
                agent.as_deref(),
                project_path.as_ref(),
            )?;
        }
        SkillsCommands::Remove {
            skill,
            agent,
            project,
        } => {
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
pub fn execute_template(
    process: &Option<String>,
    profile: &Option<String>,
    output: &Option<String>,
    ctx: &Context,
) -> Result<()> {
    let _ = ctx; // unused

    if process.is_none() {
        println!("Available device templates:");
        for (name, desc) in DeviceTemplates::list() {
            println!("  {} - {}", name, desc);
        }
        return Ok(());
    }

    let template_path = PathBuf::from(process.as_ref().unwrap());
    let output_path = output
        .as_ref()
        .map(PathBuf::from)
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
pub fn execute_init(force: bool, ctx: &Context) -> Result<()> {
    println!("Initializing Android CLI...");

    // Create storage directory
    let storage_path = ctx.sys_info.cli_storage_path();
    std::fs::create_dir_all(&storage_path)?;
    println!("Created storage: {}", storage_path.display());

    // Get user home directory
    let home = ctx.sys_info.user_home.clone();

    // Install bundled android-cli skill to multiple AI agent directories
    // Matches Google original behavior
    let agent_locations = [
        ".gemini/antigravity/skills",
        ".claude/skills",
        ".gemini/skills",
        ".config/opencode/skills",
        ".trae-cn/skills",
    ];

    for agent_path in &agent_locations {
        let skills_dir = home.join(agent_path);
        std::fs::create_dir_all(&skills_dir)?;

        let bundled_skill_source = skills_dir.join("android-cli");
        let skill_already_installed =
            bundled_skill_source.exists() && bundled_skill_source.join("SKILL.md").exists();

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
            println!(
                "Skill 'android-cli' installed to {}/{}",
                home.display(),
                agent_path
            );
        } else if skill_already_installed && !force {
            println!(
                "Skill 'android-cli' already installed to {}/{} (use --force to reinstall)",
                home.display(),
                agent_path
            );
        }
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
pub fn execute_describe(project_dir: Option<&str>, ctx: &Context) -> Result<()> {
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
pub fn execute_docs(command: Option<DocsCommands>) -> Result<()> {
    match command {
        Some(DocsCommands::Search { query }) => {
            let mut docs_cli = DocsCLI::new()?;
            let results = docs_cli.search(&query)?;
            DocsCLI::display_search_results(&results);
        }
        Some(DocsCommands::Fetch { url }) => {
            // KB-based approach doesn't support direct URL fetch
            // Use the search command to find KB documents instead
            println!("Note: The docs fetch command is not available with KB-based search.");
            println!("Use 'android docs search <query>' to search the Android Knowledge Base.");
            println!();
            println!("KB documents are downloaded locally from:");
            println!("  https://developer.android.com/static/api/kb/kb.zip");
            println!();
            println!("If you need to browse documentation online, visit:");
            println!("  {}", url);
        }
        Some(DocsCommands::Stats) => {
            let mut docs_cli = DocsCLI::new()?;
            let stats = docs_cli.stats()?;
            println!("KB Index Statistics:");
            println!("  Documents: {}", stats.num_docs);
            println!("  Index directory: {}", stats.index_dir.display());
        }
        Some(DocsCommands::Clear) => {
            let mut docs_cli = DocsCLI::new()?;
            docs_cli.clear_cache()?;
        }
        None => {
            // Default behavior when no subcommand provided
            println!("Android CLI Documentation (KB-based search):");
            println!("  Searches local Knowledge Base index for Android documentation");
            println!();
            println!("Commands:");
            println!("  android docs search <query>  - Search Android KB");
            println!("  android docs stats           - Show KB index statistics");
            println!(
                "  android docs clear           - Clear KB cache (re-download on next search)"
            );
            println!();
            println!("KB ZIP source:");
            println!("  https://developer.android.com/static/api/kb/kb.zip");
        }
    }
    Ok(())
}

// Update command
pub fn execute_update(url: Option<&str>) -> Result<()> {
    let updater = if let Some(custom_url) = url {
        Updater::with_url(custom_url)
    } else {
        Updater::new()
    };

    updater.update(url)?;

    Ok(())
}

// Info command
pub fn execute_info(field: Option<&str>, ctx: &Context) -> Result<()> {
    // Get CLI version
    let version = env!("CARGO_PKG_VERSION");

    // Get platform string
    let platform_str = match ctx.sys_info.platform {
        Platform::Mac => "macos",
        Platform::Linux | Platform::Unspecified => "linux",
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
            println!(
                "  Android Home:  {}",
                ctx.sys_info.android_user_home.display()
            );
            println!("  Platform:      {}", platform_str);
            println!("  Architecture:  {}", arch_str);
            println!("  CLI Version:   {}", version);
        }
    }

    Ok(())
}

// Create command
pub fn execute_create(
    name: Option<&str>,
    output: &str,
    min_sdk: Option<&str>,
    list: bool,
    template: Option<&str>,
    verbose: bool,
    ctx: &Context,
) -> Result<()> {
    let _ = ctx; // unused

    let mut runner = TemplateEngineRunner::new();

    if list {
        runner.print_templates()?;
        return Ok(());
    }

    let name = name.ok_or_else(|| {
        anyhow::anyhow!("The name of the application is required (e.g. 'My Application')")
    })?;

    // Default template is empty-activity
    let template_name = template.unwrap_or("empty-activity");

    let output_path = PathBuf::from(output);
    runner.create_project(template_name, name, &output_path, min_sdk, verbose)?;

    Ok(())
}

// Screen command
pub fn execute_screen(command: ScreenCommands, ctx: &Context) -> Result<()> {
    // Get device from context or auto-select (matches Kotlin behavior)
    let mut screen_cmd = ScreenCommand::new(&ctx.sdk_path)?;

    match command {
        ScreenCommands::Capture {
            output,
            annotate,
            cluster_merge_threshold,
            debug,
        } => {
            // Kotlin version uses AdbKt.getDevice which auto-selects if no device specified
            // We pass None for device to match Kotlin behavior
            screen_cmd.capture(
                None,
                output.as_deref(),
                annotate,
                cluster_merge_threshold,
                debug,
            )?;
        }
        ScreenCommands::Resolve { screenshot, string } => {
            let result = ResolveCommand::resolve(&screenshot, &string)?;
            println!("{}", result);
        }
    }

    Ok(())
}

// Layout command
pub fn execute_layout(
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
pub fn execute_help(command: Option<&str>) -> Result<()> {
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

pub fn print_command_help(cmd_name: &str) {
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
            println!();
            println!("Note: On Linux ARM (aarch64), packages are automatically downloaded from");
            println!("      https://github.com/HomuHomu833/android-sdk-custom");
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
            println!(
                "    --string                       The string to substitute coordinates into"
            );
        }
        "layout" => {
            println!("Returns the layout tree of an application");
            println!();
            println!("Usage: android layout [-o <file>] [-d] [-p] [--device <serial>]");
            println!();
            println!("Options:");
            println!("  -o, --output <file>  Writes the layout tree to the specified file");
            println!(
                "  -d, --diff           Returns flat list of elements that changed since last dump"
            );
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
            println!(
                "  -o, --output <dir>   The destination project directory path (default: '.')"
            );
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
pub fn execute_upload_metrics(ctx: &Context) -> Result<()> {
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
pub fn execute_test_metrics(command: Option<TestMetricsCommands>) -> Result<()> {
    match command {
        Some(TestMetricsCommands::Crash { thread }) => {
            if thread {
                std::thread::spawn(|| {
                    panic!("Test crash from thread");
                })
                .join()
                .unwrap();
            } else {
                panic!("Test crash");
            }
        }
        Some(TestMetricsCommands::Report {
            test_subcommand_flag: _,
        }) => {
            println!("Report invocation test (no metrics in Rust version)");
        }
        None => {
            println!("test-metrics: hidden command for testing");
        }
    }
    Ok(())
}
