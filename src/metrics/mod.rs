//! Metrics and analytics module
//!
//! Based on Kotlin AndroidCliAnalytics and related classes

use std::path::PathBuf;
use std::fs;
use std::io::Write;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Metrics configuration
pub struct MetricsConfig {
    /// Whether metrics are enabled
    enabled: bool,
    /// Metrics storage directory
    metrics_dir: PathBuf,
    /// Application version
    version: String,
}

/// Invocation metrics record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationRecord {
    /// Timestamp of invocation
    timestamp: DateTime<Utc>,
    /// Command that was invoked
    command: String,
    /// Whether the command succeeded
    success: bool,
    /// Duration in milliseconds
    duration_ms: u64,
    /// CLI version
    version: String,
    /// Platform info
    platform: String,
}

/// Crash report record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRecord {
    /// Timestamp of crash
    timestamp: DateTime<Utc>,
    /// Error message
    error: String,
    /// Stack trace (if available)
    stack_trace: Option<String>,
    /// CLI version
    version: String,
    /// Command that caused crash
    command: Option<String>,
}

impl MetricsConfig {
    /// Create metrics configuration
    pub fn new(enabled: bool, android_user_home: &PathBuf, version: &str) -> Self {
        let metrics_dir = android_user_home.join("metrics");

        Self {
            enabled,
            metrics_dir,
            version: version.to_string(),
        }
    }

    /// Check if metrics are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Ensure metrics directory exists
    pub fn ensure_dir(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        fs::create_dir_all(&self.metrics_dir)?;
        Ok(())
    }

    /// Record an invocation
    pub fn record_invocation(&self, command: &str, success: bool, duration_ms: u64, platform: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.ensure_dir()?;

        let record = InvocationRecord {
            timestamp: Utc::now(),
            command: command.to_string(),
            success,
            duration_ms,
            version: self.version.clone(),
            platform: platform.to_string(),
        };

        // Append to daily log file
        let date_str = record.timestamp.format("%Y-%m-%d").to_string();
        let log_file = self.metrics_dir.join(format!("invocations_{}.jsonl", date_str));

        let json = serde_json::to_string(&record)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;

        Ok(())
    }

    /// Record a crash
    pub fn record_crash(&self, error: &str, stack_trace: Option<&str>, command: Option<&str>) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.ensure_dir()?;

        let record = CrashRecord {
            timestamp: Utc::now(),
            error: error.to_string(),
            stack_trace: stack_trace.map(|s| s.to_string()),
            version: self.version.clone(),
            command: command.map(|s| s.to_string()),
        };

        // Append to crash log file
        let crash_file = self.metrics_dir.join("crashes.jsonl");

        let json = serde_json::to_string(&record)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_file)?;

        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;

        Ok(())
    }

    /// Get pending invocation records
    pub fn get_pending_invocations(&self) -> Result<Vec<InvocationRecord>> {
        if !self.metrics_dir.exists() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();

        for entry in fs::read_dir(&self.metrics_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if path.file_name().map(|n| n.to_string_lossy().starts_with("invocations")).unwrap_or(false) {
                    let content = fs::read_to_string(&path)?;

                    for line in content.lines() {
                        if let Ok(record) = serde_json::from_str::<InvocationRecord>(line) {
                            records.push(record);
                        }
                    }
                }
            }
        }

        Ok(records)
    }

    /// Get pending crash records
    pub fn get_pending_crashes(&self) -> Result<Vec<CrashRecord>> {
        let crash_file = self.metrics_dir.join("crashes.jsonl");

        if !crash_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&crash_file)?;
        let records: Vec<CrashRecord> = content
            .lines()
            .filter_map(|line| serde_json::from_str::<CrashRecord>(line).ok())
            .collect();

        Ok(records)
    }

    /// Clear uploaded metrics (keep uploaded files, clear pending files)
    pub fn clear_uploaded(&self) -> Result<()> {
        if !self.metrics_dir.exists() {
            return Ok(());
        }

        // Clear pending invocation files (keep uploaded files)
        for entry in fs::read_dir(&self.metrics_dir)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

            // Only clear pending files (invocations_*.jsonl), keep uploaded files (uploaded_*.json)
            if path.extension().map(|e| e == "jsonl").unwrap_or(false)
                && filename.starts_with("invocations_") {
                fs::remove_file(&path)?;
            }
        }

        // Clear pending crashes file
        let crashes_file = self.metrics_dir.join("crashes.jsonl");
        if crashes_file.exists() {
            fs::remove_file(&crashes_file)?;
        }

        Ok(())
    }
}

/// Metrics uploader - writes metrics to local files as records
pub struct MetricsUploader {
    config: MetricsConfig,
}

impl MetricsUploader {
    /// Create metrics uploader
    pub fn new(config: MetricsConfig) -> Self {
        Self { config }
    }

    /// Export pending metrics to local files
    ///
    /// Instead of uploading to a server, this writes metrics records
    /// to local JSON files for historical tracking.
    pub fn upload_now(&self) -> Result<UploadResult> {
        if !self.config.is_enabled() {
            return Ok(UploadResult {
                invocations_uploaded: 0,
                crashes_uploaded: 0,
                success: true,
                message: "Metrics disabled".to_string(),
            });
        }

        let invocations = self.config.get_pending_invocations()?;
        let crashes = self.config.get_pending_crashes()?;
        let crash_count = crashes.len();

        self.config.ensure_dir()?;

        // Write invocations to uploaded file
        let date_str = Utc::now().format("%Y-%m-%d").to_string();
        let uploaded_invocations_file = self.config.metrics_dir.join(format!("uploaded_invocations_{}.json", date_str));

        let invocations_json = serde_json::to_string_pretty(&invocations)?;
        fs::write(&uploaded_invocations_file, &invocations_json)?;

        println!("Exported metrics to local files:");
        println!("  Invocations: {} records -> {}", invocations.len(), uploaded_invocations_file.display());

        // Write crashes to uploaded file
        if !crashes.is_empty() {
            let uploaded_crashes_file = self.config.metrics_dir.join("uploaded_crashes.json");

            // Append to existing crashes file if it exists
            let existing_crashes: Vec<CrashRecord> = if uploaded_crashes_file.exists() {
                let content = fs::read_to_string(&uploaded_crashes_file)?;
                serde_json::from_str(&content).unwrap_or(Vec::new())
            } else {
                Vec::new()
            };

            let all_crashes: Vec<CrashRecord> = existing_crashes.into_iter().chain(crashes.into_iter()).collect();
            let crashes_json = serde_json::to_string_pretty(&all_crashes)?;
            fs::write(&uploaded_crashes_file, &crashes_json)?;

            println!("  Crashes: {} reports -> {}", all_crashes.len(), uploaded_crashes_file.display());
        }

        let result = UploadResult {
            invocations_uploaded: invocations.len(),
            crashes_uploaded: crash_count,
            success: true,
            message: "Metrics exported to local files successfully".to_string(),
        };

        // Clear uploaded records after successful export
        if result.success {
            self.config.clear_uploaded()?;
        }

        Ok(result)
    }

    /// Export crash reports to local file
    pub fn upload_crash_reports(&self) -> Result<()> {
        if !self.config.is_enabled() {
            return Ok(());
        }

        let crashes = self.config.get_pending_crashes()?;
        let crash_count = crashes.len();

        if crashes.is_empty() {
            println!("No crash reports to export");
            return Ok(());
        }

        self.config.ensure_dir()?;

        let uploaded_crashes_file = self.config.metrics_dir.join("uploaded_crashes.json");

        // Append to existing crashes file if it exists
        let existing_crashes: Vec<CrashRecord> = if uploaded_crashes_file.exists() {
            let content = fs::read_to_string(&uploaded_crashes_file)?;
            serde_json::from_str(&content).unwrap_or(Vec::new())
        } else {
            Vec::new()
        };

        let all_crashes: Vec<CrashRecord> = existing_crashes.into_iter().chain(crashes.into_iter()).collect();
        let crashes_json = serde_json::to_string_pretty(&all_crashes)?;
        fs::write(&uploaded_crashes_file, &crashes_json)?;

        println!("Exported {} crash reports to {}", crash_count, uploaded_crashes_file.display());

        Ok(())
    }
}

/// Upload result
#[derive(Debug)]
pub struct UploadResult {
    pub invocations_uploaded: usize,
    pub crashes_uploaded: usize,
    pub success: bool,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_metrics_config_disabled() {
        let dir = tempdir().unwrap();
        let config = MetricsConfig::new(false, &PathBuf::from(dir.path()), "0.1.0");

        assert!(!config.is_enabled());

        // Recording should silently skip when disabled
        config.record_invocation("test", true, 100, "test_platform").unwrap();

        let invocations = config.get_pending_invocations().unwrap();
        assert!(invocations.is_empty());
    }

    #[test]
    fn test_invocation_record_serialization() {
        let record = InvocationRecord {
            timestamp: Utc::now(),
            command: "sdk install".to_string(),
            success: true,
            duration_ms: 1234,
            version: "0.1.0".to_string(),
            platform: "darwin".to_string(),
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("sdk install"));

        let parsed: InvocationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "sdk install");
        assert_eq!(parsed.duration_ms, 1234);
    }

    #[test]
    fn test_crash_record_serialization() {
        let record = CrashRecord {
            timestamp: Utc::now(),
            error: "Test error".to_string(),
            stack_trace: Some("stack trace here".to_string()),
            version: "0.1.0".to_string(),
            command: Some("test command".to_string()),
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("Test error"));

        let parsed: CrashRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error, "Test error");
        assert!(parsed.stack_trace.is_some());
    }

    #[test]
    fn test_upload_result_creation() {
        let result = UploadResult {
            invocations_uploaded: 10,
            crashes_uploaded: 2,
            success: true,
            message: "OK".to_string(),
        };

        assert_eq!(result.invocations_uploaded, 10);
        assert!(result.success);
    }

    #[test]
    fn test_upload_now_writes_local_files() {
        let dir = tempdir().unwrap();
        let config = MetricsConfig::new(true, &PathBuf::from(dir.path()), "0.1.0");

        // Record some invocations
        config.record_invocation("sdk install", true, 100, "darwin").unwrap();
        config.record_invocation("screen capture", true, 200, "darwin").unwrap();

        // Record a crash
        config.record_crash("Test error", Some("stack trace"), Some("test")).unwrap();

        let uploader = MetricsUploader::new(config);
        let result = uploader.upload_now().unwrap();

        assert!(result.success);
        assert_eq!(result.invocations_uploaded, 2);
        assert_eq!(result.crashes_uploaded, 1);

        // Check that uploaded files exist
        let metrics_dir = dir.path().join("metrics");
        assert!(metrics_dir.exists());

        // Find uploaded invocations file
        let uploaded_invocations = fs::read_dir(&metrics_dir)
            .unwrap()
            .find(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("uploaded_invocations"))
            .unwrap()
            .unwrap()
            .path();

        let content = fs::read_to_string(&uploaded_invocations).unwrap();
        let uploaded_records: Vec<InvocationRecord> = serde_json::from_str(&content).unwrap();
        assert_eq!(uploaded_records.len(), 2);

        // Check uploaded crashes file
        let uploaded_crashes = metrics_dir.join("uploaded_crashes.json");
        assert!(uploaded_crashes.exists());

        let crashes_content = fs::read_to_string(&uploaded_crashes).unwrap();
        let uploaded_crashes_records: Vec<CrashRecord> = serde_json::from_str(&crashes_content).unwrap();
        assert_eq!(uploaded_crashes_records.len(), 1);
    }

    #[test]
    fn test_clear_uploaded_removes_pending_files() {
        let dir = tempdir().unwrap();
        let config = MetricsConfig::new(true, &PathBuf::from(dir.path()), "0.1.0");

        // Record some invocations
        config.record_invocation("test", true, 100, "darwin").unwrap();

        // Check pending file exists
        let metrics_dir = dir.path().join("metrics");
        let pending_exists = fs::read_dir(&metrics_dir)
            .unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().starts_with("invocations_"));

        assert!(pending_exists);

        // Clear uploaded
        config.clear_uploaded().unwrap();

        // Check pending files are removed
        let pending_after = fs::read_dir(&metrics_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name().to_string_lossy().starts_with("invocations_")
                && e.path().extension().map(|ext| ext == "jsonl").unwrap_or(false)
            });

        assert!(!pending_after);
    }
}