//! Skills installation location for different AI agents
//!
//! Based on Kotlin SkillsInstallLocation enum

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;

/// Skills installation location for different AI agents
///
/// Each agent has:
/// - agentName: identifier for the agent
/// - globalPath: path for global installation (in user home)
/// - projectPath: path for project-level installation
/// - description: human-readable description
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillsInstallLocation {
    /// AdaL agent
    Adal,
    /// Antigravity agent
    Antigravity,
    /// Augment agent
    Augment,
    /// IBM Bob agent
    Bob,
    /// Claude Code agent
    ClaudeCode,
    /// Common - shared by multiple agents
    Common,
    /// CodeBuddy agent
    Codebuddy,
    /// Codex agent
    Codex,
    /// Command Code agent
    CommandCode,
    /// Continue agent
    Continue,
    /// Cortex Code agent
    CortexCode,
    /// Crush agent
    Crush,
    /// Cursor agent
    Cursor,
    /// Deep Agents agent
    DeepAgents,
    /// Droid agent
    Droid,
    /// Firebender agent
    Firebender,
    /// Gemini CLI agent
    Gemini,
    /// GitHub Copilot agent
    GithubCopilot,
    /// Goose agent
    Goose,
    /// iFlow CLI agent
    Iflow,
    /// Junie agent
    Junie,
    /// Kilo Code agent
    KiloCode,
    /// Kiro CLI agent
    Kiro,
    /// Kode agent
    Kode,
    /// MCPJam agent
    Mcpjam,
    /// Mistral Vibe agent
    MistralVibe,
    /// Mux agent
    Mux,
    /// Neovate agent
    Neovate,
    /// OpenClaw agent
    Openclaw,
    /// OpenCode agent
    Opencode,
    /// OpenHands agent
    Openhands,
    /// Pi agent
    Pi,
    /// Pochi agent
    Pochi,
    /// Qoder agent
    Qoder,
    /// Qwen Code agent
    QwenCode,
    /// Roo Code agent
    RooCode,
    /// Trae agent
    Trae,
    /// Trae CN agent
    TraeCn,
    /// Universal agent
    Universal,
    /// Windsurf agent
    Windsurf,
    /// Zencoder agent
    Zencoder,
}

impl SkillsInstallLocation {
    /// Get the agent name (identifier)
    pub fn agent_name(&self) -> &'static str {
        match self {
            SkillsInstallLocation::Adal => "adal",
            SkillsInstallLocation::Antigravity => "antigravity",
            SkillsInstallLocation::Augment => "augment",
            SkillsInstallLocation::Bob => "bob",
            SkillsInstallLocation::ClaudeCode => "claude-code",
            SkillsInstallLocation::Common => "common",
            SkillsInstallLocation::Codebuddy => "codebuddy",
            SkillsInstallLocation::Codex => "codex",
            SkillsInstallLocation::CommandCode => "command-code",
            SkillsInstallLocation::Continue => "continue",
            SkillsInstallLocation::CortexCode => "cortex-code",
            SkillsInstallLocation::Crush => "crush",
            SkillsInstallLocation::Cursor => "cursor",
            SkillsInstallLocation::DeepAgents => "deep-agents",
            SkillsInstallLocation::Droid => "droid",
            SkillsInstallLocation::Firebender => "firebender",
            SkillsInstallLocation::Gemini => "gemini",
            SkillsInstallLocation::GithubCopilot => "github-copilot",
            SkillsInstallLocation::Goose => "goose",
            SkillsInstallLocation::Iflow => "iflow",
            SkillsInstallLocation::Junie => "junie",
            SkillsInstallLocation::KiloCode => "kilo-code",
            SkillsInstallLocation::Kiro => "kiro",
            SkillsInstallLocation::Kode => "kode",
            SkillsInstallLocation::Mcpjam => "mcpjam",
            SkillsInstallLocation::MistralVibe => "mistral-vibe",
            SkillsInstallLocation::Mux => "mux",
            SkillsInstallLocation::Neovate => "neovate",
            SkillsInstallLocation::Openclaw => "openclaw",
            SkillsInstallLocation::Opencode => "opencode",
            SkillsInstallLocation::Openhands => "openhands",
            SkillsInstallLocation::Pi => "pi",
            SkillsInstallLocation::Pochi => "pochi",
            SkillsInstallLocation::Qoder => "qoder",
            SkillsInstallLocation::QwenCode => "qwen-code",
            SkillsInstallLocation::RooCode => "roo-code",
            SkillsInstallLocation::Trae => "trae",
            SkillsInstallLocation::TraeCn => "trae-cn",
            SkillsInstallLocation::Universal => "universal",
            SkillsInstallLocation::Windsurf => "windsurf",
            SkillsInstallLocation::Zencoder => "zencoder",
        }
    }

    /// Get the global installation path (relative to user home)
    pub fn global_path(&self) -> &'static str {
        match self {
            SkillsInstallLocation::Adal => ".adal/skills/",
            SkillsInstallLocation::Antigravity => ".gemini/antigravity/skills/",
            SkillsInstallLocation::Augment => ".augment/skills/",
            SkillsInstallLocation::Bob => ".bob/skills/",
            SkillsInstallLocation::ClaudeCode => ".claude/skills/",
            SkillsInstallLocation::Common => ".agents/skills/",
            SkillsInstallLocation::Codebuddy => ".codebuddy/skills/",
            SkillsInstallLocation::Codex => ".codex/skills/",
            SkillsInstallLocation::CommandCode => ".commandcode/skills/",
            SkillsInstallLocation::Continue => ".continue/skills/",
            SkillsInstallLocation::CortexCode => ".snowflake/cortex/skills/",
            SkillsInstallLocation::Crush => ".config/crush/skills/",
            SkillsInstallLocation::Cursor => ".cursor/skills/",
            SkillsInstallLocation::DeepAgents => ".deepagents/agent/skills/",
            SkillsInstallLocation::Droid => ".factory/skills/",
            SkillsInstallLocation::Firebender => ".firebender/skills/",
            SkillsInstallLocation::Gemini => ".gemini/skills/",
            SkillsInstallLocation::GithubCopilot => ".copilot/skills/",
            SkillsInstallLocation::Goose => ".config/goose/skills/",
            SkillsInstallLocation::Iflow => ".iflow/skills/",
            SkillsInstallLocation::Junie => ".junie/skills/",
            SkillsInstallLocation::KiloCode => ".kilo/skills/",
            SkillsInstallLocation::Kiro => ".kiro/skills/",
            SkillsInstallLocation::Kode => ".kode/skills/",
            SkillsInstallLocation::Mcpjam => ".mcpjam/skills/",
            SkillsInstallLocation::MistralVibe => ".vibe/skills/",
            SkillsInstallLocation::Mux => ".mux/skills/",
            SkillsInstallLocation::Neovate => ".neovate/skills/",
            SkillsInstallLocation::Openclaw => ".openclaw/skills/",
            SkillsInstallLocation::Opencode => ".config/opencode/skills/",
            SkillsInstallLocation::Openhands => ".openhands/skills/",
            SkillsInstallLocation::Pi => ".pi/agent/skills/",
            SkillsInstallLocation::Pochi => ".pochi/skills/",
            SkillsInstallLocation::Qoder => ".qoder/skills/",
            SkillsInstallLocation::QwenCode => ".qwen/skills/",
            SkillsInstallLocation::RooCode => ".roo/skills/",
            SkillsInstallLocation::Trae => ".trae/skills/",
            SkillsInstallLocation::TraeCn => ".trae-cn/skills/",
            SkillsInstallLocation::Universal => ".config/agents/skills/",
            SkillsInstallLocation::Windsurf => ".codeium/windsurf/skills/",
            SkillsInstallLocation::Zencoder => ".zencoder/skills/",
        }
    }

    /// Get the project-level installation path (relative to project root)
    pub fn project_path(&self) -> &'static str {
        match self {
            SkillsInstallLocation::Adal => ".adal/skills/",
            SkillsInstallLocation::Antigravity => ".agents/skills/",
            SkillsInstallLocation::Augment => ".augment/skills/",
            SkillsInstallLocation::Bob => ".bob/skills/",
            SkillsInstallLocation::ClaudeCode => ".claude/skills/",
            SkillsInstallLocation::Common => ".agents/skills/",
            SkillsInstallLocation::Codebuddy => ".codebuddy/skills/",
            SkillsInstallLocation::Codex => ".agents/skills/",
            SkillsInstallLocation::CommandCode => ".commandcode/skills/",
            SkillsInstallLocation::Continue => ".continue/skills/",
            SkillsInstallLocation::CortexCode => ".cortex/skills/",
            SkillsInstallLocation::Crush => ".crush/skills/",
            SkillsInstallLocation::Cursor => ".agents/skills/",
            SkillsInstallLocation::DeepAgents => ".agents/skills/",
            SkillsInstallLocation::Droid => ".factory/skills/",
            SkillsInstallLocation::Firebender => ".agents/skills/",
            SkillsInstallLocation::Gemini => ".agents/skills/",
            SkillsInstallLocation::GithubCopilot => ".agents/skills/",
            SkillsInstallLocation::Goose => ".goose/skills/",
            SkillsInstallLocation::Iflow => ".iflow/skills/",
            SkillsInstallLocation::Junie => ".junie/skills/",
            SkillsInstallLocation::KiloCode => ".kilocode/skills/",
            SkillsInstallLocation::Kiro => ".kiro/skills/",
            SkillsInstallLocation::Kode => ".kode/skills/",
            SkillsInstallLocation::Mcpjam => ".mcpjam/skills/",
            SkillsInstallLocation::MistralVibe => ".vibe/skills/",
            SkillsInstallLocation::Mux => ".mux/skills/",
            SkillsInstallLocation::Neovate => ".neovate/skills/",
            SkillsInstallLocation::Openclaw => "skills/",
            SkillsInstallLocation::Opencode => ".agents/skills/",
            SkillsInstallLocation::Openhands => ".openhands/skills/",
            SkillsInstallLocation::Pi => ".pi/skills/",
            SkillsInstallLocation::Pochi => ".pochi/skills/",
            SkillsInstallLocation::Qoder => ".qoder/skills/",
            SkillsInstallLocation::QwenCode => ".qwen/skills/",
            SkillsInstallLocation::RooCode => ".roo/skills/",
            SkillsInstallLocation::Trae => ".trae/skills/",
            SkillsInstallLocation::TraeCn => ".trae/skills/",
            SkillsInstallLocation::Universal => ".agents/skills/",
            SkillsInstallLocation::Windsurf => ".windsurf/skills/",
            SkillsInstallLocation::Zencoder => ".zencoder/skills/",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            SkillsInstallLocation::Adal => "Supports AdaL",
            SkillsInstallLocation::Antigravity => "Supports Antigravity",
            SkillsInstallLocation::Augment => "Supports Augment",
            SkillsInstallLocation::Bob => "Supports IBM Bob",
            SkillsInstallLocation::ClaudeCode => "Supports Claude Code",
            SkillsInstallLocation::Common => "Supports Copilot, Codex, OpenClaw, and others",
            SkillsInstallLocation::Codebuddy => "Supports CodeBuddy",
            SkillsInstallLocation::Codex => "Supports Codex",
            SkillsInstallLocation::CommandCode => "Supports Command Code",
            SkillsInstallLocation::Continue => "Supports Continue",
            SkillsInstallLocation::CortexCode => "Supports Cortex Code",
            SkillsInstallLocation::Crush => "Supports Crush",
            SkillsInstallLocation::Cursor => "Supports Cursor",
            SkillsInstallLocation::DeepAgents => "Supports Deep Agents",
            SkillsInstallLocation::Droid => "Supports Droid",
            SkillsInstallLocation::Firebender => "Supports Firebender",
            SkillsInstallLocation::Gemini => "Supports Gemini CLI",
            SkillsInstallLocation::GithubCopilot => "Supports GitHub Copilot",
            SkillsInstallLocation::Goose => "Supports Goose",
            SkillsInstallLocation::Iflow => "Supports iFlow CLI",
            SkillsInstallLocation::Junie => "Supports Junie",
            SkillsInstallLocation::KiloCode => "Supports Kilo Code",
            SkillsInstallLocation::Kiro => "Supports Kiro CLI",
            SkillsInstallLocation::Kode => "Supports Kode",
            SkillsInstallLocation::Mcpjam => "Supports MCPJam",
            SkillsInstallLocation::MistralVibe => "Supports Mistral Vibe",
            SkillsInstallLocation::Mux => "Supports Mux",
            SkillsInstallLocation::Neovate => "Supports Neovate",
            SkillsInstallLocation::Openclaw => "Supports OpenClaw",
            SkillsInstallLocation::Opencode => "Supports OpenCode",
            SkillsInstallLocation::Openhands => "Supports OpenHands",
            SkillsInstallLocation::Pi => "Supports Pi",
            SkillsInstallLocation::Pochi => "Supports Pochi",
            SkillsInstallLocation::Qoder => "Supports Qoder",
            SkillsInstallLocation::QwenCode => "Supports Qwen Code",
            SkillsInstallLocation::RooCode => "Supports Roo Code",
            SkillsInstallLocation::Trae => "Supports Trae",
            SkillsInstallLocation::TraeCn => "Supports Trae CN",
            SkillsInstallLocation::Universal => "Supports Amp, Kimi Code CLI, Replit, Universal",
            SkillsInstallLocation::Windsurf => "Supports Windsurf",
            SkillsInstallLocation::Zencoder => "Supports Zencoder",
        }
    }

    /// Get the full installation path (absolute)
    pub fn get_install_root(&self, base_dir: &Path, is_project: bool) -> PathBuf {
        let path = if is_project {
            self.project_path()
        } else {
            self.global_path()
        };
        base_dir.join(path)
    }

    /// Get all locations
    pub fn all() -> &'static [SkillsInstallLocation] {
        &[
            SkillsInstallLocation::Adal,
            SkillsInstallLocation::Antigravity,
            SkillsInstallLocation::Augment,
            SkillsInstallLocation::Bob,
            SkillsInstallLocation::ClaudeCode,
            SkillsInstallLocation::Common,
            SkillsInstallLocation::Codebuddy,
            SkillsInstallLocation::Codex,
            SkillsInstallLocation::CommandCode,
            SkillsInstallLocation::Continue,
            SkillsInstallLocation::CortexCode,
            SkillsInstallLocation::Crush,
            SkillsInstallLocation::Cursor,
            SkillsInstallLocation::DeepAgents,
            SkillsInstallLocation::Droid,
            SkillsInstallLocation::Firebender,
            SkillsInstallLocation::Gemini,
            SkillsInstallLocation::GithubCopilot,
            SkillsInstallLocation::Goose,
            SkillsInstallLocation::Iflow,
            SkillsInstallLocation::Junie,
            SkillsInstallLocation::KiloCode,
            SkillsInstallLocation::Kiro,
            SkillsInstallLocation::Kode,
            SkillsInstallLocation::Mcpjam,
            SkillsInstallLocation::MistralVibe,
            SkillsInstallLocation::Mux,
            SkillsInstallLocation::Neovate,
            SkillsInstallLocation::Openclaw,
            SkillsInstallLocation::Opencode,
            SkillsInstallLocation::Openhands,
            SkillsInstallLocation::Pi,
            SkillsInstallLocation::Pochi,
            SkillsInstallLocation::Qoder,
            SkillsInstallLocation::QwenCode,
            SkillsInstallLocation::RooCode,
            SkillsInstallLocation::Trae,
            SkillsInstallLocation::TraeCn,
            SkillsInstallLocation::Universal,
            SkillsInstallLocation::Windsurf,
            SkillsInstallLocation::Zencoder,
        ]
    }

    /// Get location by agent name
    pub fn from_agent_name(name: &str) -> Option<Self> {
        for location in Self::all() {
            if location.agent_name() == name {
                return Some(*location);
            }
        }
        None
    }

    /// Get map of agent names to locations
    pub fn by_agent_name() -> HashMap<&'static str, SkillsInstallLocation> {
        Self::all().iter().map(|l| (l.agent_name(), *l)).collect()
    }

    /// Get existing installation locations
    ///
    /// Returns a list of locations where the skills directory exists
    pub fn get_existing_locations(base_dir: &Path, is_project: bool) -> Vec<SkillsInstallLocation> {
        Self::all()
            .iter()
            .filter(|location| {
                let path = location.get_install_root(base_dir, is_project);
                // Check if directory exists or parent directory exists
                path.exists() || path.parent().map(|p| p.exists()).unwrap_or(false)
            })
            .copied()
            .collect()
    }

    /// Parse agent string (comma-separated) into locations
    ///
    /// Returns a list of valid SkillsInstallLocation
    /// Throws error if invalid agent name
    pub fn parse_agents(agent_string: Option<&str>) -> Result<Vec<SkillsInstallLocation>, String> {
        if agent_string.is_none() {
            return Ok(Vec::new());
        }

        let agent_str = agent_string.unwrap();
        let mut locations = Vec::new();

        for agent_name in agent_str.split(',') {
            let trimmed = agent_name.trim();
            if let Some(location) = Self::from_agent_name(trimmed) {
                locations.push(location);
            } else {
                let valid_agents: Vec<&str> = Self::all().iter().map(|l| l.agent_name()).collect();
                return Err(format!(
                    "Invalid agent: {}. Valid options are: {}",
                    trimmed,
                    valid_agents.join(", ")
                ));
            }
        }

        Ok(locations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_names() {
        assert_eq!(SkillsInstallLocation::ClaudeCode.agent_name(), "claude-code");
        assert_eq!(SkillsInstallLocation::Gemini.agent_name(), "gemini");
        assert_eq!(SkillsInstallLocation::Cursor.agent_name(), "cursor");
    }

    #[test]
    fn test_global_paths() {
        assert_eq!(SkillsInstallLocation::ClaudeCode.global_path(), ".claude/skills/");
        assert_eq!(SkillsInstallLocation::Gemini.global_path(), ".gemini/skills/");
    }

    #[test]
    fn test_project_paths() {
        assert_eq!(SkillsInstallLocation::ClaudeCode.project_path(), ".claude/skills/");
        assert_eq!(SkillsInstallLocation::Codex.project_path(), ".agents/skills/");
    }

    #[test]
    fn test_from_agent_name() {
        assert_eq!(
            SkillsInstallLocation::from_agent_name("claude-code"),
            Some(SkillsInstallLocation::ClaudeCode)
        );
        assert_eq!(
            SkillsInstallLocation::from_agent_name("invalid"),
            None
        );
    }

    #[test]
    fn test_parse_agents() {
        let result = SkillsInstallLocation::parse_agents(Some("claude-code,gemini"));
        assert!(result.is_ok());
        let locations = result.unwrap();
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0], SkillsInstallLocation::ClaudeCode);
        assert_eq!(locations[1], SkillsInstallLocation::Gemini);
    }

    #[test]
    fn test_parse_agents_none() {
        let result = SkillsInstallLocation::parse_agents(None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_agents_invalid() {
        let result = SkillsInstallLocation::parse_agents(Some("invalid-agent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_all_locations() {
        let all = SkillsInstallLocation::all();
        assert!(all.len() >= 40); // Should have at least 40 agents
    }

    #[test]
    fn test_by_agent_name_map() {
        let map = SkillsInstallLocation::by_agent_name();
        assert!(map.contains_key("claude-code"));
        assert!(map.contains_key("gemini"));
    }
}