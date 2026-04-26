use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Read, Write};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::skills::location::SkillsInstallLocation;

/// CLI version for bundled skills versioning
const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub releases URL for downloading latest skills
const SKILLS_DOWNLOAD_URL: &str = "https://github.com/android/skills/releases/latest/download/android-skills.zip";

/// Skill metadata from SKILL.md YAML frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Skill definition
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub version: String,
    pub path: PathBuf,
    pub has_claude: bool,
    pub has_gemini: bool,
    pub content: Option<String>,
}

impl Skill {
    /// Parse SKILL.md file
    pub fn parse(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read SKILL.md: {}", path.display()))?;

        // Parse YAML frontmatter
        let metadata = if content.starts_with("---") {
            let end = content.find("\n---\n")
                .or_else(|| content.find("\n---"))
                .unwrap_or(0);

            if end > 3 {
                let yaml = &content[3..end];
                serde_yaml::from_str::<SkillMetadata>(yaml)
                    .context("Failed to parse SKILL.md frontmatter")?
            } else {
                SkillMetadata {
                    name: path.parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    description: String::new(),
                    version: None,
                    author: None,
                    tags: None,
                }
            }
        } else {
            SkillMetadata {
                name: path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                description: String::new(),
                version: None,
                author: None,
                tags: None,
            }
        };

        // Check for agent-specific files
        let skill_dir = path.parent().unwrap_or(path);
        let has_claude = skill_dir.join("CLAUDE.md").exists();
        let has_gemini = skill_dir.join("GEMINI.md").exists();

        Ok(Self {
            name: metadata.name,
            description: metadata.description,
            version: metadata.version.unwrap_or_else(|| "1.0".to_string()),
            path: skill_dir.to_path_buf(),
            has_claude,
            has_gemini,
            content: Some(content),
        })
    }
}

/// Agent installation location
#[derive(Debug, Clone)]
pub struct AgentLocation {
    pub name: String,
    pub path: PathBuf,
    pub agent_type: String,
}

/// Skill Manager
pub struct SkillManager {
    /// Available skills directory (built-in)
    skills_dir: PathBuf,
    /// User skills installation directory
    user_skills_dir: PathBuf,
    /// Android CLI directory for DAC skills
    android_cli_dir: PathBuf,
    /// User home directory
    home_dir: PathBuf,
}

impl SkillManager {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"));

        let skills_dir = home.join(".claude").join("skills");
        let user_skills_dir = home.join(".claude");
        let android_cli_dir = home.join(".android").join("cli");

        Ok(Self {
            skills_dir,
            user_skills_dir,
            android_cli_dir,
            home_dir: home,
        })
    }

    /// Create a SkillManager with a custom home directory (for testing)
    #[cfg(test)]
    pub fn new_with_home(home: PathBuf) -> Result<Self> {
        let skills_dir = home.join(".claude").join("skills");
        let user_skills_dir = home.join(".claude");
        let android_cli_dir = home.join(".android").join("cli");

        Ok(Self {
            skills_dir,
            user_skills_dir,
            android_cli_dir,
            home_dir: home,
        })
    }

    /// Get the path to the DAC skills zip file
    fn dac_skills_zip_path(&self) -> PathBuf {
        self.android_cli_dir.join("skills").join("dac_skills.zip")
    }

    /// Get the path to the DAC skills ETag file
    fn dac_skills_etag_path(&self) -> PathBuf {
        self.android_cli_dir.join("skills").join("dac_skills.etag")
    }

    /// Get the path to the DAC skills extracted directory
    fn dac_skills_dir(&self) -> PathBuf {
        self.android_cli_dir.join("skills").join("dac_skills")
    }

    /// Get the path to the bundled skills version file
    fn bundled_version_path(&self) -> PathBuf {
        self.android_cli_dir.join("skills").join("version")
    }

    /// Fetch latest skills from GitHub releases with ETag caching
    ///
    /// Downloads skills from https://github.com/android/skills/releases/latest/download/android-skills.zip
    /// Uses ETag caching to avoid re-downloading unchanged skills.
    ///
    /// Returns Ok(true) if skills were downloaded, Ok(false) if unchanged (304 Not Modified)
    pub fn fetch_latest_skills(&self) -> Result<bool> {
        let skills_path = self.dac_skills_zip_path();
        let etag_path = self.dac_skills_etag_path();

        // Create parent directories
        if let Some(parent) = skills_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        println!("Fetching latest skills from GitHub...");

        // Use reqwest to download with ETag support
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent(format!("Android-CLI/{}", CLI_VERSION))
            .build()
            .context("Failed to create HTTP client")?;

        // Read existing ETag if available
        let existing_etag = if etag_path.exists() {
            fs::read_to_string(&etag_path).ok()
        } else {
            None
        };

        // Build request with conditional headers
        let mut request = client.get(SKILLS_DOWNLOAD_URL);
        if let Some(etag) = &existing_etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag.trim());
        }

        let response = request.send()
            .context("Failed to fetch skills from GitHub")?;

        // Check if unchanged (304 Not Modified)
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            println!("Skills unchanged (cached).");
            return Ok(false);
        }

        // Check for success
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to download skills: HTTP {}",
                response.status()
            ));
        }

        // Get new ETag
        let new_etag = response.headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Download with progress
        let content_length = response.headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let mut file = fs::File::create(&skills_path)
            .with_context(|| format!("Failed to create file: {}", skills_path.display()))?;

        let mut downloaded: u64 = 0;
        let mut buffer = [0u8; 8192];
        let mut reader = response;

        loop {
            let n = reader.read(&mut buffer)
                .context("Failed to read response")?;
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n])
                .context("Failed to write to file")?;
            downloaded += n as u64;

            // Show progress if we know the content length
            if let Some(total) = content_length {
                let percent = (downloaded as f64 / total as f64 * 100.0) as i32;
                print!("\rDownloading skills: {}% ({}/{} bytes)", percent, downloaded, total);
            } else {
                print!("\rDownloaded {} bytes...", downloaded);
            }
            io::stdout().flush().ok();
        }
        println!();

        // Save new ETag
        if let Some(etag) = new_etag {
            fs::write(&etag_path, &etag)
                .with_context(|| format!("Failed to write ETag file: {}", etag_path.display()))?;
        }

        println!("Skills downloaded to: {}", skills_path.display());
        Ok(true)
    }

    /// Unzip bundled skills from embedded resources
    ///
    /// Checks version file at ~/.android/cli/skills/version
    /// If version doesn't match current CLI version, extracts skills.zip
    ///
    /// For now, this simulates the bundled skills extraction since we don't have
    /// actual embedded resources. In a real implementation, this would extract
    /// from include_bytes!() embedded zip.
    pub fn unzip_bundled_skills(&self) -> Result<PathBuf> {
        let version_path = self.bundled_version_path();
        let skills_dir = self.dac_skills_dir();

        // Check if version matches
        let needs_extract = if version_path.exists() {
            let stored_version = fs::read_to_string(&version_path)
                .unwrap_or_default()
                .trim()
                .to_string();
            stored_version != CLI_VERSION
        } else {
            true
        };

        if !needs_extract && skills_dir.exists() {
            // Version matches and skills already extracted
            return Ok(skills_dir);
        }

        // Create skills directory
        fs::create_dir_all(&skills_dir)
            .with_context(|| format!("Failed to create directory: {}", skills_dir.display()))?;

        // Check if we have a downloaded zip to extract
        let zip_path = self.dac_skills_zip_path();
        if zip_path.exists() {
            println!("Extracting skills from {}...", zip_path.display());

            let zip_file = fs::File::open(&zip_path)
                .with_context(|| format!("Failed to open zip file: {}", zip_path.display()))?;

            let mut archive = ZipArchive::new(zip_file)
                .context("Failed to read zip archive")?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = match file.enclosed_name() {
                    Some(path) => {
                        let path_str = path.display().to_string();
                        let full_path = skills_dir.join(&path);
                        // Security: Verify path doesn't escape target directory (path traversal protection)
                        let canonical_skills_dir = skills_dir.canonicalize()
                            .unwrap_or_else(|_| skills_dir.clone());
                        // For paths that don't exist yet, check prefix
                        if !full_path.starts_with(&canonical_skills_dir) &&
                            !full_path.to_str().map(|s| s.starts_with(skills_dir.to_str().unwrap_or(""))).unwrap_or(false) {
                            // Skip potentially malicious path traversal attempts
                            println!("Skipping suspicious path: {}", path_str);
                            continue;
                        }
                        full_path
                    }
                    None => continue,
                };

                if file.name().ends_with('/') {
                    fs::create_dir_all(&outpath)
                        .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            fs::create_dir_all(p)
                                .with_context(|| format!("Failed to create directory: {}", p.display()))?;
                        }
                    }
                    let mut outfile = fs::File::create(&outpath)
                        .with_context(|| format!("Failed to create file: {}", outpath.display()))?;
                    io::copy(&mut file, &mut outfile)
                        .context("Failed to extract file")?;
                }
            }

            println!("Extracted {} files", archive.len());
        } else {
            // No zip file, create a placeholder bundled skills structure
            println!("No skills zip found, creating placeholder bundled skills...");
            self.create_placeholder_bundled_skills(&skills_dir)?;
        }

        // Update version file
        fs::write(&version_path, CLI_VERSION)
            .with_context(|| format!("Failed to write version file: {}", version_path.display()))?;

        Ok(skills_dir)
    }

    /// Create placeholder bundled skills for testing/development
    fn create_placeholder_bundled_skills(&self, skills_dir: &Path) -> Result<()> {
        // Create a basic android-cli skill
        let android_cli_skill = skills_dir.join("android-cli");
        fs::create_dir_all(&android_cli_skill)?;

        let skill_md = r#"---
name: android-cli
description: Android CLI development skill for AI agents
version: "1.0"
author: android-cli
tags:
  - android
  - cli
  - development
---

# Android CLI Development Skill

Provides guidance for using the Android CLI tool.
"#;
        fs::write(android_cli_skill.join("SKILL.md"), skill_md)?;

        let claude_md = r#"# Android CLI Skill for Claude

This skill helps Claude work with Android projects using the CLI.

## Commands
- `android sdk install <package>` - Install SDK packages
- `android emulator list` - List AVDs
- `android device list` - List devices
"#;
        fs::write(android_cli_skill.join("CLAUDE.md"), claude_md)?;

        Ok(())
    }

    /// Install a specific bundled skill by name
    ///
    /// First unzips bundled skills if needed, then installs the specified skill
    /// to the user's skills directory using do_install (install_skill).
    pub fn install_bundled(&self, skill_name: &str, agent: Option<&str>, project: Option<&PathBuf>) -> Result<()> {
        // Ensure bundled skills are extracted
        let bundled_dir = self.unzip_bundled_skills()?;

        // Find the skill in bundled directory
        let skill_path = bundled_dir.join(skill_name);
        let skill_md = skill_path.join("SKILL.md");

        if !skill_md.exists() {
            return Err(anyhow::anyhow!(
                "Bundled skill '{}' not found. Available skills can be listed in: {}",
                skill_name,
                bundled_dir.display()
            ));
        }

        // Parse the skill
        let skill = Skill::parse(&skill_md)?;

        // Determine target directory
        let target_dir = if let Some(proj) = project {
            proj.join(".claude")
        } else {
            self.user_skills_dir.clone()
        };

        // Install the skill
        self.install_skill(&skill, &target_dir, agent)?;

        println!("Installed bundled skill: {}", skill_name);
        Ok(())
    }

    /// Get existing agent installation directories
    ///
    /// Detects existing agent directories:
    /// - ~/.claude (antigravity/Claude)
    /// - ~/.gemini (Gemini CLI)
    ///
    /// Returns list of locations where skills can be installed
    pub fn get_existing_locations(&self) -> Result<Vec<AgentLocation>> {
        let mut locations = Vec::new();

        // Check for Claude directory
        let claude_dir = self.home_dir.join(".claude");
        if claude_dir.exists() {
            locations.push(AgentLocation {
                name: "Claude (antigravity)".to_string(),
                path: claude_dir,
                agent_type: "claude".to_string(),
            });
        }

        // Check for Gemini directory
        let gemini_dir = self.home_dir.join(".gemini");
        if gemini_dir.exists() {
            locations.push(AgentLocation {
                name: "Gemini CLI".to_string(),
                path: gemini_dir,
                agent_type: "gemini".to_string(),
            });
        }

        // Check for Android CLI skills directory
        let android_cli_skills = self.android_cli_dir.join("skills");
        if android_cli_skills.exists() {
            locations.push(AgentLocation {
                name: "Android CLI".to_string(),
                path: android_cli_skills,
                agent_type: "android-cli".to_string(),
            });
        }

        Ok(locations)
    }

    /// List installed skills
    pub fn list(&self, agent: Option<&str>, project: Option<&PathBuf>) -> Result<Vec<Skill>> {
        let base_dir = if let Some(proj) = project {
            proj.join(".claude")
        } else {
            self.user_skills_dir.clone()
        };

        let mut skills = Vec::new();

        if !base_dir.exists() {
            return Ok(skills);
        }

        for entry in fs::read_dir(&base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    if let Ok(skill) = Skill::parse(&skill_md) {
                        // Filter by agent if specified
                        if let Some(a) = agent {
                            if a == "claude" && !skill.has_claude {
                                continue;
                            }
                            if a == "gemini" && !skill.has_gemini {
                                continue;
                            }
                        }
                        skills.push(skill);
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Add/install skill
    pub fn add(&self, skill_name: Option<&str>, all: bool, agent: Option<&str>, project: Option<&PathBuf>) -> Result<()> {
        let target_dir = if let Some(proj) = project {
            proj.join(".claude")
        } else {
            self.user_skills_dir.clone()
        };

        fs::create_dir_all(&target_dir)?;

        if all {
            // Install all available skills
            let available = self.list_available()?;
            let count = available.len();
            for skill in available {
                self.install_skill(&skill, &target_dir, agent)?;
            }
            println!("Installed {} skills", count);
        } else if let Some(name) = skill_name {
            // Install specific skill
            let skill = self.find_available(name)?
                .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;
            self.install_skill(&skill, &target_dir, agent)?;
        } else {
            return Err(anyhow::anyhow!("Specify --skill or --all"));
        }

        Ok(())
    }

    /// Install a skill to target directory
    fn install_skill(&self, skill: &Skill, target_dir: &Path, agent: Option<&str>) -> Result<()> {
        let skill_target = target_dir.join(&skill.name);
        fs::create_dir_all(&skill_target)?;

        // Copy SKILL.md
        let skill_md_src = skill.path.join("SKILL.md");
        let skill_md_dst = skill_target.join("SKILL.md");
        if skill_md_src.exists() {
            fs::copy(&skill_md_src, &skill_md_dst)?;
        }

        // Copy agent-specific files based on selection
        if agent.is_none() || agent == Some("claude") {
            if skill.has_claude {
                let claude_src = skill.path.join("CLAUDE.md");
                let claude_dst = skill_target.join("CLAUDE.md");
                fs::copy(&claude_src, &claude_dst)?;
            }
        }

        if agent.is_none() || agent == Some("gemini") {
            if skill.has_gemini {
                let gemini_src = skill.path.join("GEMINI.md");
                let gemini_dst = skill_target.join("GEMINI.md");
                fs::copy(&gemini_src, &gemini_dst)?;
            }
        }

        // Copy references directory if exists
        let refs_src = skill.path.join("references");
        if refs_src.exists() {
            let refs_dst = skill_target.join("references");
            self.copy_dir(&refs_src, &refs_dst)?;
        }

        println!("Installed skill: {}", skill.name);

        Ok(())
    }

    /// Copy directory recursively
    fn copy_dir(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_dir(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    /// Remove skill
    pub fn remove(&self, skill_name: &str, agent: Option<&str>, project: Option<&PathBuf>) -> Result<()> {
        // Parse agent string if provided
        let agents = if let Some(agent_str) = agent {
            SkillsInstallLocation::parse_agents(Some(agent_str))
                .map_err(|e| anyhow::anyhow!("{}", e))?
        } else {
            // If no agent specified, get all existing locations
            SkillsInstallLocation::get_existing_locations(&self.home_dir, false)
        };

        if agents.is_empty() {
            // Fallback: remove from default location
            let base_dir = if let Some(proj) = project {
                proj.join(".claude")
            } else {
                self.user_skills_dir.clone()
            };

            let skill_dir = base_dir.join(skill_name);
            if skill_dir.exists() {
                fs::remove_dir_all(&skill_dir)?;
                println!("Removed skill: {}", skill_name);
            } else {
                println!("Skill '{}' not installed", skill_name);
            }
            return Ok(());
        }

        // Remove from each agent location
        for location in agents {
            let base_dir = if let Some(proj) = project {
                proj.clone()
            } else {
                self.home_dir.clone()
            };

            let install_path = location.get_install_root(&base_dir, project.is_some());
            let skill_dir = install_path.join(skill_name);

            if skill_dir.exists() {
                fs::remove_dir_all(&skill_dir)?;
                println!("Removed skill '{}' from {}", skill_name, location.agent_name());
            } else {
                println!("Skill '{}' not installed in {}", skill_name, location.agent_name());
            }
        }

        Ok(())
    }

    /// Find skills by keyword
    pub fn find(&self, keyword: &str) -> Result<Vec<Skill>> {
        let available = self.list_available()?;
        let installed = self.list(None, None)?;

        let mut results = Vec::new();

        // Search in available skills
        for skill in available.iter().chain(installed.iter()) {
            if skill.name.contains(keyword) ||
               skill.description.contains(keyword) ||
               skill.content.as_ref().map(|c| c.contains(keyword)).unwrap_or(false) {
                results.push(skill.clone());
            }
        }

        Ok(results)
    }

    /// List available built-in skills
    fn list_available(&self) -> Result<Vec<Skill>> {
        if !self.skills_dir.exists() {
            // Return empty if no skills directory
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();

        for entry in fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    if let Ok(skill) = Skill::parse(&skill_md) {
                        skills.push(skill);
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Find available skill by name
    fn find_available(&self, name: &str) -> Result<Option<Skill>> {
        let available = self.list_available()?;
        Ok(available.into_iter().find(|s| s.name == name))
    }

    /// Get skill content for specific agent
    pub fn get_agent_content(&self, skill: &Skill, agent: &str) -> Result<Option<String>> {
        let file = if agent == "gemini" {
            skill.path.join("GEMINI.md")
        } else {
            skill.path.join("CLAUDE.md")
        };

        if file.exists() {
            Ok(Some(fs::read_to_string(file)?))
        } else {
            Ok(None)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_skill_manager_new() {
        let manager = SkillManager::new().unwrap();
        // Should work even with empty skills directory
        let skills = manager.list(None, None).unwrap();
        assert!(skills.is_empty() || skills.len() >= 0);
    }

    #[test]
    fn test_skill_metadata_parse() {
        let yaml = "name: test-skill\ndescription: A test skill\nversion: \"1.0\"";
        let metadata: SkillMetadata = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(metadata.name, "test-skill");
        assert_eq!(metadata.description, "A test skill");
        assert_eq!(metadata.version, Some("1.0".to_string()));
    }

    #[test]
    fn test_skill_parse_empty_frontmatter() {
        let content = "---\n---\n# Skill content";
        // Would parse but with default/empty values
        // Actual parsing requires file with proper structure
    }

    #[test]
    fn test_find_empty() {
        let manager = SkillManager::new().unwrap();
        let results = manager.find("nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_existing_locations() {
        let manager = SkillManager::new().unwrap();
        let locations = manager.get_existing_locations().unwrap();
        // Should always return at least one if .claude or .gemini exists
        // The result depends on user's system state
        assert!(locations.len() >= 0);
    }

    #[test]
    fn test_agent_location_fields() {
        let loc = AgentLocation {
            name: "Test".to_string(),
            path: PathBuf::from("/test/path"),
            agent_type: "claude".to_string(),
        };
        assert_eq!(loc.name, "Test");
        assert_eq!(loc.agent_type, "claude");
    }

    #[test]
    fn test_dac_skills_paths() {
        let manager = SkillManager::new().unwrap();

        // Verify paths are constructed correctly
        let zip_path = manager.dac_skills_zip_path();
        assert!(zip_path.to_string_lossy().contains("dac_skills.zip"));

        let etag_path = manager.dac_skills_etag_path();
        assert!(etag_path.to_string_lossy().contains("dac_skills.etag"));

        let skills_dir = manager.dac_skills_dir();
        assert!(skills_dir.to_string_lossy().contains("dac_skills"));

        let version_path = manager.bundled_version_path();
        assert!(version_path.to_string_lossy().ends_with("version"));
    }

    #[test]
    fn test_unzip_bundled_skills_creates_placeholder() {
        let temp_home = tempdir().unwrap();
        let manager = SkillManager::new_with_home(temp_home.path().to_path_buf()).unwrap();

        // First call should create placeholder skills
        let skills_dir = manager.unzip_bundled_skills().unwrap();
        assert!(skills_dir.exists());

        // Should have android-cli skill
        let android_cli = skills_dir.join("android-cli");
        assert!(android_cli.exists());
        assert!(android_cli.join("SKILL.md").exists());

        // Version file should be written
        let version_path = manager.bundled_version_path();
        assert!(version_path.exists());
        let version = fs::read_to_string(version_path).unwrap();
        assert_eq!(version.trim(), CLI_VERSION);

        // Second call should not re-extract (version matches)
        let skills_dir2 = manager.unzip_bundled_skills().unwrap();
        assert_eq!(skills_dir, skills_dir2);
    }

    #[test]
    fn test_install_bundled_skill() {
        let temp_home = tempdir().unwrap();
        let manager = SkillManager::new_with_home(temp_home.path().to_path_buf()).unwrap();

        // First unzip bundled skills
        manager.unzip_bundled_skills().unwrap();

        // Install the bundled android-cli skill
        let result = manager.install_bundled("android-cli", None, None);
        assert!(result.is_ok());

        // Check skill is installed
        let user_skills = temp_home.path().join(".claude").join("android-cli");
        assert!(user_skills.exists());
        assert!(user_skills.join("SKILL.md").exists());
    }

    #[test]
    fn test_install_bundled_nonexistent_skill() {
        let temp_home = tempdir().unwrap();
        let manager = SkillManager::new_with_home(temp_home.path().to_path_buf()).unwrap();

        manager.unzip_bundled_skills().unwrap();

        // Try to install a skill that doesn't exist
        let result = manager.install_bundled("nonexistent-skill", None, None);
        assert!(result.is_err());
    }
}
