use std::path::Path;
use std::fs;
use std::io::Read;
use std::collections::HashMap;
use anyhow::{Result, Context, bail};
use zip::ZipArchive;
use serde::{Deserialize, Serialize};

/// Embedded templates.zip from Google Android CLI
const TEMPLATE_ZIP: &[u8] = include_bytes!("templates/templates.zip");

/// Template engine for creating Android projects
pub struct TemplateEngineRunner {
    /// Cached template list
    template_list: Option<TemplateList>,
    /// Cached template directory prefixes (short_name -> prefix path)
    template_prefixes: HashMap<String, String>,
}

/// Template list parsed from templates.zip
#[derive(Debug, Clone)]
struct TemplateList {
    templates: Vec<TemplateDefinition>,
}

/// Template definition from template-definition.json
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplateDefinition {
    name: String,
    #[serde(rename = "short-name")]
    short_name: String,
    tags: Vec<String>,
    arguments: Vec<TemplateArgument>,
    #[serde(default)]
    dependencies: Vec<TemplateDependency>,
    #[serde(default)]
    transformations: Vec<TemplateTransformation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplateArgument {
    id: String,
    description: Option<String>,
    #[serde(rename = "default-value")]
    default_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplateDependency {
    #[serde(rename = "sdk-package")]
    sdk_package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum TemplateTransformation {
    StringReplace {
        #[serde(rename = "string-replace")]
        string_replace: StringReplaceConfig,
    },
    RenameFile {
        #[serde(rename = "rename-file")]
        rename_file: RenameFileConfig,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StringReplaceConfig {
    description: Option<String>,
    selector: Selector,
    from: String,
    to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenameFileConfig {
    description: Option<String>,
    selector: Selector,
    #[serde(rename = "source-path")]
    source_path: String,
    #[serde(rename = "target-path")]
    target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Selector {
    glob: String,
}

/// Template information for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    pub short_name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub is_default: bool,
}

impl TemplateEngineRunner {
    /// Create new template engine runner
    pub fn new() -> Self {
        Self {
            template_list: None,
            template_prefixes: HashMap::new(),
        }
    }

    /// Load template list from embedded templates.zip and cache prefixes
    fn load_template_list(&mut self) -> Result<&TemplateList> {
        if self.template_list.is_some() {
            return Ok(self.template_list.as_ref().unwrap());
        }

        let mut archive = ZipArchive::new(std::io::Cursor::new(TEMPLATE_ZIP))
            .context("Failed to open embedded templates.zip")?;

        let mut templates = Vec::new();
        let mut prefixes = HashMap::new();

        // Find all template directories (look for template-definition.json)
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let path = file.name().to_string();  // Clone to avoid borrow issues

            if path.contains("template-definition.json") {
                let mut content = Vec::new();
                file.read_to_end(&mut content)?;
                let content_str = String::from_utf8_lossy(&content).into_owned();
                let definition: TemplateDefinition = serde_json::from_str(&content_str)
                    .context("Failed to parse template-definition.json")?;

                // Extract prefix up to gradle version
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 4 {
                    // Format: {short-name}/gradle/{version}/...
                    let prefix = format!("{}/{}/{}/", parts[0], parts[1], parts[2]);
                    prefixes.insert(definition.short_name.clone(), prefix);
                }

                templates.push(definition);
            }
        }

        self.template_list = Some(TemplateList { templates });
        self.template_prefixes = prefixes;
        Ok(self.template_list.as_ref().unwrap())
    }

    /// List available templates (matches Google original)
    pub fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        let mut runner = Self::new();
        let list = runner.load_template_list()?;

        let templates: Vec<TemplateInfo> = list.templates.iter().map(|t| {
            TemplateInfo {
                name: t.name.clone(),
                short_name: t.short_name.clone(),
                description: t.name.clone(),
                tags: t.tags.clone(),
                is_default: t.short_name == "empty-activity",
            }
        }).collect();

        Ok(templates)
    }

    /// Print available templates
    pub fn print_templates(&self) -> Result<()> {
        let templates = self.list_templates()?;
        println!("Template name                 Template description    Tags");
        for template in &templates {
            let default_marker = if template.is_default { " (default)" } else { "" };
            println!("{}{}    {}    {}",
                template.short_name,
                default_marker,
                template.name,
                template.tags.join(",")
            );
        }
        Ok(())
    }

    /// Create project from template
    pub fn create_project(
        &mut self,
        template_name: &str,
        project_name: &str,
        output_dir: &Path,
        min_sdk: Option<&str>,
        verbose: bool,
    ) -> Result<()> {
        // Default template is empty-activity
        let template_short_name = if template_name.is_empty() {
            "empty-activity"
        } else {
            template_name
        };

        // Find and clone template first to avoid borrow issues
        let template = self.find_template(template_short_name)?
            .ok_or_else(|| anyhow::anyhow!(
                "Unknown template name '{}', use 'android create --list' to see list of available templates",
                template_short_name
            ))?;

        if project_name.is_empty() {
            bail!("The name of the application is required (e.g. 'My Application')");
        }

        // Build template arguments
        let mut args: HashMap<String, String> = HashMap::new();

        for arg in &template.arguments {
            let value = match arg.id.as_str() {
                "name" => project_name.to_string(),
                "minSdk" => min_sdk.map(|s| s.to_string())
                    .unwrap_or_else(|| evaluate_template_expr(&arg.default_value, &args)),
                _ => evaluate_template_expr(&arg.default_value, &args),
            };
            args.insert(arg.id.clone(), value);
        }

        // Add sdkPath if available
        if let Ok(sdk_path) = std::env::var("ANDROID_HOME") {
            args.insert("sdkPath".to_string(), sdk_path);
        } else if let Ok(sdk_path) = std::env::var("ANDROID_SDK_ROOT") {
            args.insert("sdkPath".to_string(), sdk_path);
        }

        let compile_sdk = args.get("compileSdk").cloned().unwrap_or_else(|| "36".to_string());
        args.insert("compileSdk".to_string(), compile_sdk);

        if verbose {
            println!("INFO: Processing template '{}'", template.name);
            println!("VERBOSE: Effective arguments values: {{sdkPath={}, name={}, applicationId={}, namespace={}, minSdk={}, compileSdk={}}}",
                args.get("sdkPath").unwrap_or(&"<unknown>".to_string()),
                args.get("name").unwrap_or(&"".to_string()),
                args.get("applicationId").unwrap_or(&"".to_string()),
                args.get("namespace").unwrap_or(&"".to_string()),
                args.get("minSdk").unwrap_or(&"24".to_string()),
                args.get("compileSdk").unwrap_or(&"36".to_string()));
        }

        // Create project directory
        let project_dir = output_dir.join(sanitize_project_name(&args.get("name").unwrap_or(&"app".to_string())));
        if project_dir.exists() {
            bail!(
                "Directory '{}' already exists. Please choose a different name or output directory.",
                project_dir.display()
            );
        }
        fs::create_dir_all(&project_dir)
            .with_context(|| format!("Failed to create project directory: {}", project_dir.display()))?;

        // Get cached template prefix
        let template_prefix = self.template_prefixes.get(template_short_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Template prefix not found for '{}'", template_short_name))?;

        // Process template
        self.process_template(&template, &template_prefix, &args, &project_dir, verbose)?;

        if verbose {
            println!("INFO: Successfully created project '{}' at '{}'",
                template.name, project_dir.display());
        } else {
            println!("Created project: {}", project_dir.display());
        }

        Ok(())
    }

    /// Find template by short name and clone it
    fn find_template(&mut self, short_name: &str) -> Result<Option<TemplateDefinition>> {
        let list = self.load_template_list()?;
        let template = list.templates.iter()
            .find(|t| t.short_name == short_name)
            .cloned();
        Ok(template)
    }

    /// Process template: copy files and apply transformations
    fn process_template(
        &self,
        template: &TemplateDefinition,
        template_prefix: &str,
        args: &HashMap<String, String>,
        project_dir: &Path,
        verbose: bool,
    ) -> Result<()> {
        let mut archive = ZipArchive::new(std::io::Cursor::new(TEMPLATE_ZIP))
            .context("Failed to open embedded templates.zip")?;

        // Copy all files from template
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let path = file.name().to_string();  // Clone path to avoid borrow issues

            // Skip template-definition.json and directories
            if path.ends_with("template-definition.json") || path.ends_with('/') {
                continue;
            }

            // Check if file belongs to this template
            if !path.starts_with(&template_prefix) {
                continue;
            }

            // Extract relative path (remove template prefix)
            let relative_path = path.strip_prefix(&template_prefix).unwrap_or(&path).to_string();

            // Determine target path
            let mut target_path = project_dir.join(&relative_path);

            // Read file content
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;

            // Apply string transformations
            let mut content_str = if is_text_file(&relative_path) {
                String::from_utf8_lossy(&content).into_owned()
            } else {
                // Binary file, no transformations
                if verbose {
                    println!("VERBOSE: Template file '{}': Copying contents unchanged", relative_path);
                }
                fs::create_dir_all(target_path.parent().unwrap())?;
                fs::write(&target_path, &content)?;
                continue;
            };

            // Apply transformations in order
            for transform in &template.transformations {
                match transform {
                    TemplateTransformation::StringReplace { string_replace } => {
                        // Check both original and renamed path
                        let check_path = target_path.strip_prefix(project_dir)
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_else(|_| relative_path.clone());

                        if matches_glob(&string_replace.selector.glob, &check_path) ||
                           matches_glob(&string_replace.selector.glob, &relative_path) {
                            let from = evaluate_template_expr(&string_replace.from, args);
                            let to = evaluate_template_expr(&string_replace.to, args);
                            content_str = content_str.replace(&from, &to);
                        }
                    }
                    TemplateTransformation::RenameFile { rename_file } => {
                        if matches_glob(&rename_file.selector.glob, &relative_path) {
                            let source = evaluate_template_expr(&rename_file.source_path, args);
                            let target = evaluate_template_expr(&rename_file.target_path, args);

                            // Check if path contains source (for directory renaming like com/example/myapplication)
                            // or filename matches source (for exact file renaming like local.properties.template)
                            let filename = relative_path.rsplit('/').next().unwrap_or(&relative_path);
                            if relative_path.contains(&source) || filename == source {
                                let new_relative = relative_path.replace(&source, &target);
                                target_path = project_dir.join(new_relative);
                            }
                        }
                    }
                }
            }

            // Write file
            if verbose {
                println!("VERBOSE: Template file '{}': Renaming file to '{}'",
                    relative_path, target_path.file_name().unwrap().to_string_lossy());
                println!("VERBOSE: Saving destination file '{}' ({} byte(s))",
                    target_path.display(), content_str.len());
            }

            fs::create_dir_all(target_path.parent().unwrap())?;
            fs::write(&target_path, content_str)?;
        }

        Ok(())
    }
}

impl Default for TemplateEngineRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if file is text file (needs transformation)
fn is_text_file(path: &str) -> bool {
    let text_extensions = [".kt", ".kts", ".java", ".xml", ".json", ".toml",
                           ".properties", ".properties.template", ".txt", ".gradle",
                           ".md", ".gitignore", ".sh", ".bat"];
    text_extensions.iter().any(|ext| path.ends_with(ext))
}

/// Check if path matches glob pattern
fn matches_glob(glob: &str, path: &str) -> bool {
    // Simple glob matching
    let normalized_glob = glob.trim_start_matches('/');
    let normalized_path = path.trim_start_matches('/');

    if normalized_glob == "**/*" || normalized_glob == "*" {
        return true;
    }

    // Handle patterns like "**/*.kt"
    if normalized_glob.starts_with("**/*.") {
        let ext = normalized_glob.strip_prefix("**/*").unwrap();
        return normalized_path.ends_with(ext);
    }

    if normalized_glob.starts_with("**/") {
        let suffix = normalized_glob.strip_prefix("**/").unwrap();
        return normalized_path == suffix || normalized_path.ends_with(&format!("/{}", suffix));
    }

    if normalized_glob.starts_with('/') {
        return normalized_path == normalized_glob.strip_prefix('/').unwrap();
    }

    // Handle patterns like "*.kt" (match filename)
    if normalized_glob.starts_with("*.") {
        let ext = normalized_glob.strip_prefix("*").unwrap();
        // Extract filename from path and check extension
        let filename = normalized_path.rsplit('/').next().unwrap_or(normalized_path);
        return filename.ends_with(ext);
    }

    normalized_path.contains(normalized_glob)
}

/// Evaluate template expression (handle ${var} and ${var.method()})
fn evaluate_template_expr(expr: &str, args: &HashMap<String, String>) -> String {
    let mut result = expr.to_string();

    // Replace simple ${var}
    for (key, value) in args {
        result = result.replace(&format!("${{{}}}", key), value);
    }

    // Handle special method calls
    // ${namespace.replace('.','/')}
    if result.contains("${namespace.replace('.','/')") {
        let namespace = args.get("namespace").cloned().unwrap_or_default();
        let replaced = namespace.replace('.', "/");
        result = result.replace("${namespace.replace('.','/')}", &replaced);
    }

    // ${name.toAndroidPackageSegment()}
    if result.contains("${name.toAndroidPackageSegment()") {
        let name = args.get("name").cloned().unwrap_or_default();
        let sanitized = name.replace(' ', "").replace('-', "").replace('_', "").to_lowercase();
        result = result.replace("${name.toAndroidPackageSegment()}", &sanitized);
    }

    // ${name.toJavaPackageSegment()}
    if result.contains("${name.toJavaPackageSegment()") {
        let name = args.get("name").cloned().unwrap_or_default();
        let sanitized = name.replace(' ', "").replace('-', "").replace('_', "").to_lowercase();
        result = result.replace("${name.toJavaPackageSegment()}", &sanitized);
    }

    // ${sdkPath.toJavaPropertyValue()}
    if result.contains("${sdkPath.toJavaPropertyValue()") {
        let sdk_path = args.get("sdkPath").cloned().unwrap_or_default();
        result = result.replace("${sdkPath.toJavaPropertyValue()}", &sdk_path);
    }

    result
}

/// Sanitize project name for directory
fn sanitize_project_name(name: &str) -> String {
    name.replace(' ', "_")
        .replace('-', "_")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_list_templates() {
        let runner = TemplateEngineRunner::new();
        let templates = runner.list_templates().unwrap();
        assert!(!templates.is_empty());
        assert!(templates.iter().any(|t| t.short_name == "empty-activity"));
    }

    #[test]
    fn test_default_template() {
        let runner = TemplateEngineRunner::new();
        let templates = runner.list_templates().unwrap();
        let default = templates.iter().find(|t| t.is_default);
        assert!(default.is_some());
        assert_eq!(default.unwrap().short_name, "empty-activity");
    }

    #[test]
    fn test_create_project() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("empty-activity", "TestApp", dir.path(), None, false).unwrap();

        assert!(dir.path().join("testapp").exists());
        assert!(dir.path().join("testapp/build.gradle.kts").exists());
        assert!(dir.path().join("testapp/app/build.gradle.kts").exists());
        assert!(dir.path().join("testapp/gradle/libs.versions.toml").exists());
    }

    #[test]
    fn test_create_project_default_template() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        // Empty template name should use default
        runner.create_project("", "TestApp", dir.path(), None, false).unwrap();

        assert!(dir.path().join("testapp").exists());
    }

    #[test]
    fn test_create_project_unknown_template() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        let result = runner.create_project("unknown", "TestApp", dir.path(), None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_template_file_structure() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("empty-activity", "TestApp", dir.path(), None, false).unwrap();

        let project_dir = dir.path().join("testapp");

        // Check root files
        assert!(project_dir.join("build.gradle.kts").exists());
        assert!(project_dir.join("settings.gradle.kts").exists());
        assert!(project_dir.join("gradle.properties").exists());
        assert!(project_dir.join("gradle/libs.versions.toml").exists());
        assert!(project_dir.join("gradlew").exists());

        // Check app module
        assert!(project_dir.join("app/build.gradle.kts").exists());
        assert!(project_dir.join("app/src/main/AndroidManifest.xml").exists());

        // Check Kotlin sources (path should be transformed based on namespace)
        let src_dir = project_dir.join("app/src/main/java/com/example/testapp");
        assert!(src_dir.join("MainActivity.kt").exists());
        assert!(src_dir.join("Navigation.kt").exists());
        assert!(src_dir.join("NavigationKeys.kt").exists());
        assert!(src_dir.join("theme/Color.kt").exists());
        assert!(src_dir.join("theme/Theme.kt").exists());
        assert!(src_dir.join("theme/Type.kt").exists());
        assert!(src_dir.join("data/DataRepository.kt").exists());
        assert!(src_dir.join("ui/main/MainScreen.kt").exists());
        assert!(src_dir.join("ui/main/MainScreenViewModel.kt").exists());

        // Check test files
        assert!(project_dir.join("app/src/test/java/com/example/testapp/ui/main/MainScreenViewModelTest.kt").exists());
        assert!(project_dir.join("app/src/androidTest/java/com/example/testapp/ui/main/MainScreenTest.kt").exists());

        // Check resources
        assert!(project_dir.join("app/src/main/res/values/strings.xml").exists());
    }

    #[test]
    fn test_string_replacement() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("empty-activity", "My Cool App", dir.path(), None, false).unwrap();

        // Check strings.xml has correct app name
        let strings = std::fs::read_to_string(dir.path().join("my_cool_app/app/src/main/res/values/strings.xml")).unwrap();
        assert!(strings.contains("My Cool App"));

        // Check settings.gradle.kts has correct project name
        let settings = std::fs::read_to_string(dir.path().join("my_cool_app/settings.gradle.kts")).unwrap();
        assert!(settings.contains("My Cool App"));
    }

    #[test]
    fn test_namespace_replacement() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("empty-activity", "TestApp", dir.path(), None, false).unwrap();

        // Check build.gradle.kts has correct namespace
        let build_gradle = std::fs::read_to_string(dir.path().join("testapp/app/build.gradle.kts")).unwrap();
        assert!(build_gradle.contains("namespace = \"com.example.testapp\""));
        assert!(build_gradle.contains("applicationId = \"com.example.testapp\""));
    }

    #[test]
    fn test_custom_min_sdk() {
        let mut runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("empty-activity", "TestApp", dir.path(), Some("26"), false).unwrap();

        let build_gradle = std::fs::read_to_string(dir.path().join("testapp/app/build.gradle.kts")).unwrap();
        assert!(build_gradle.contains("minSdk = 26"));
    }

    #[test]
    fn test_matches_glob() {
        assert!(matches_glob("**/*.kt", "MainActivity.kt"));
        assert!(matches_glob("**/*.kt", "theme/Color.kt"));
        assert!(matches_glob("/settings.gradle.kts", "settings.gradle.kts"));
        assert!(matches_glob("**/*", "any/path/file.txt"));
    }

    #[test]
    fn test_evaluate_template_expr() {
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Test App".to_string());
        args.insert("namespace".to_string(), "com.example.testapp".to_string());

        // Simple replacement
        assert_eq!(evaluate_template_expr("${name}", &args), "Test App");

        // Method replacement
        assert_eq!(evaluate_template_expr("${namespace.replace('.','/')}", &args), "com/example/testapp");
    }
}