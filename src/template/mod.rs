use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Template processor for device configuration
pub struct TemplateProcessor {
    /// Template variables
    variables: HashMap<String, String>,
}

impl TemplateProcessor {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Set variable
    pub fn set(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_string(), value.to_string());
    }

    /// Set multiple variables
    pub fn set_vars(&mut self, vars: HashMap<String, String>) {
        self.variables.extend(vars);
    }

    /// Process template content
    pub fn process(&self, content: &str) -> Result<String> {
        let mut result = content.to_string();

        // Replace {{variable}} patterns
        let re = Regex::new(r"\{\{(\w+)\}\}").context("Failed to create regex")?;

        for cap in re.captures_iter(content) {
            if let Some(var_name) = cap.get(1) {
                let name = var_name.as_str();
                if let Some(value) = self.variables.get(name) {
                    result = result.replace(&format!("{{{{{}}}}}", name), value);
                }
            }
        }

        // Replace ${variable} patterns
        let re2 = Regex::new(r"\$\{(\w+)\}").context("Failed to create regex")?;

        let replacements: Vec<(String, String)> = re2
            .captures_iter(&result)
            .filter_map(|cap| {
                cap.get(1).and_then(|var_name| {
                    let name = var_name.as_str();
                    self.variables
                        .get(name)
                        .map(|value| (format!("${{{}}}", name), value.clone()))
                })
            })
            .collect();

        for (pattern, value) in replacements {
            result = result.replace(&pattern, &value);
        }

        Ok(result)
    }

    /// Process template file
    pub fn process_file(&self, template_path: &Path, output_path: &Path) -> Result<()> {
        let content = fs::read_to_string(template_path)
            .with_context(|| format!("Failed to read template: {}", template_path.display()))?;

        let processed = self.process(&content)?;

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(output_path, processed)
            .with_context(|| format!("Failed to write output: {}", output_path.display()))?;

        println!(
            "Processed: {} -> {}",
            template_path.display(),
            output_path.display()
        );

        Ok(())
    }

    /// Process template directory
    pub fn process_dir(&self, template_dir: &Path, output_dir: &Path) -> Result<()> {
        fs::create_dir_all(output_dir)?;

        for entry in fs::read_dir(template_dir)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = output_dir.join(entry.file_name());

            if src_path.is_dir() {
                self.process_dir(&src_path, &dst_path)?;
            } else {
                // Process file if it looks like a template
                let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("");

                if ext == "template" || ext == "tpl" || ext.ends_with(".template") {
                    // Remove template extension
                    let new_name = dst_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.replace(".template", "").replace(".tpl", ""))
                        .unwrap_or_else(|| {
                            dst_path.file_name().unwrap().to_string_lossy().to_string()
                        });

                    let final_path = dst_path.parent().unwrap().join(new_name);
                    self.process_file(&src_path, &final_path)?;
                } else {
                    // Just copy non-template files
                    fs::copy(&src_path, &dst_path)?;
                }
            }
        }

        Ok(())
    }
}

/// Device template presets
pub struct DeviceTemplates;

impl DeviceTemplates {
    /// Get available device templates
    pub fn list() -> Vec<(&'static str, &'static str)> {
        vec![
            ("pixel_6", "Pixel 6 - 1080x2400, API 33"),
            ("pixel_6_pro", "Pixel 6 Pro - 1440x3120, API 33"),
            ("pixel_5", "Pixel 5 - 1080x2340, API 31"),
            ("pixel_4", "Pixel 4 - 1080x2280, API 29"),
            ("nexus_6", "Nexus 6 - 2560x1440, API 23"),
            ("nexus_5x", "Nexus 5X - 1080x1920, API 23"),
            ("nexus_9", "Nexus 9 - 2048x1536, API 23"),
            ("tv_1080p", "Android TV - 1920x1080, API 31"),
            ("tv_4k", "Android TV 4K - 3840x2160, API 31"),
            ("wear_square", "Wear OS Square - 320x320, API 30"),
            ("wear_round", "Wear OS Round - 320x320, API 30"),
            ("generic_phone", "Generic Phone - 480x800, API 28"),
            ("generic_tablet_7", "Generic Tablet 7\" - 800x1280, API 28"),
            (
                "generic_tablet_10",
                "Generic Tablet 10\" - 1280x800, API 28",
            ),
        ]
    }

    /// Get template config for device
    pub fn get_config(name: &str) -> Option<HashMap<String, String>> {
        let configs: HashMap<&str, HashMap<&str, &str>> = [
            (
                "pixel_6",
                HashMap::from([
                    ("device", "pixel_6"),
                    ("display_width", "1080"),
                    ("display_height", "2400"),
                    ("density", "420"),
                    ("api_level", "33"),
                    ("ram", "8192"),
                ]),
            ),
            (
                "pixel_6_pro",
                HashMap::from([
                    ("device", "pixel_6_pro"),
                    ("display_width", "1440"),
                    ("display_height", "3120"),
                    ("density", "560"),
                    ("api_level", "33"),
                    ("ram", "12288"),
                ]),
            ),
            (
                "pixel_5",
                HashMap::from([
                    ("device", "pixel_5"),
                    ("display_width", "1080"),
                    ("display_height", "2340"),
                    ("density", "440"),
                    ("api_level", "31"),
                    ("ram", "8192"),
                ]),
            ),
            (
                "nexus_6",
                HashMap::from([
                    ("device", "nexus_6"),
                    ("display_width", "2560"),
                    ("display_height", "1440"),
                    ("density", "560"),
                    ("api_level", "23"),
                    ("ram", "3072"),
                ]),
            ),
            (
                "generic_phone",
                HashMap::from([
                    ("device", "generic"),
                    ("display_width", "480"),
                    ("display_height", "800"),
                    ("density", "240"),
                    ("api_level", "28"),
                    ("ram", "2048"),
                ]),
            ),
            (
                "generic_tablet_7",
                HashMap::from([
                    ("device", "generic_tablet"),
                    ("display_width", "800"),
                    ("display_height", "1280"),
                    ("density", "160"),
                    ("api_level", "28"),
                    ("ram", "4096"),
                ]),
            ),
        ]
        .iter()
        .cloned()
        .collect();

        configs.get(name).map(|c| {
            c.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_template_simple_variable() {
        let mut processor = TemplateProcessor::new();
        processor.set("name", "Android");

        let result = processor.process("Hello {{name}}!").unwrap();
        assert_eq!(result, "Hello Android!");
    }

    #[test]
    fn test_template_multiple_variables() {
        let mut processor = TemplateProcessor::new();
        processor.set("first", "Hello");
        processor.set("second", "World");

        let result = processor.process("{{first}} {{second}}!").unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_template_dollar_variable() {
        let mut processor = TemplateProcessor::new();
        processor.set("var", "test");

        let result = processor.process("Value: ${var}").unwrap();
        assert_eq!(result, "Value: test");
    }

    #[test]
    fn test_template_missing_variable() {
        let processor = TemplateProcessor::new();
        let result = processor.process("Hello {{missing}}!").unwrap();
        assert_eq!(result, "Hello {{missing}}!"); // Unchanged
    }

    #[test]
    fn test_template_no_variables() {
        let processor = TemplateProcessor::new();
        let result = processor.process("No variables here").unwrap();
        assert_eq!(result, "No variables here");
    }

    #[test]
    fn test_device_templates_list() {
        let templates = DeviceTemplates::list();
        assert!(templates.len() >= 10);
    }

    #[test]
    fn test_device_templates_config_pixel_6() {
        let config = DeviceTemplates::get_config("pixel_6");
        assert!(config.is_some());

        let cfg = config.unwrap();
        assert_eq!(cfg.get("display_width"), Some(&"1080".to_string()));
        assert_eq!(cfg.get("api_level"), Some(&"33".to_string()));
    }

    #[test]
    fn test_device_templates_config_unknown() {
        let config = DeviceTemplates::get_config("unknown_device");
        assert!(config.is_none());
    }

    #[test]
    fn test_template_file() {
        let dir = tempdir().unwrap();
        let template_file = dir.path().join("test.template");
        let output_file = dir.path().join("test.txt");

        fs::write(&template_file, "Hello {{name}}!").unwrap();

        let mut processor = TemplateProcessor::new();
        processor.set("name", "World");
        processor
            .process_file(&template_file, &output_file)
            .unwrap();

        let result = fs::read_to_string(&output_file).unwrap();
        assert_eq!(result, "Hello World!");
    }
}
