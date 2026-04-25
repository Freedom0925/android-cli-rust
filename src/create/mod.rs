use std::path::{Path, PathBuf};
use std::fs;
use std::io::{Cursor, Read, Write};
use anyhow::{Result, Context, bail};
use zip::ZipArchive;
use serde::{Deserialize, Serialize};

/// Template engine for creating Android projects
pub struct TemplateEngineRunner {
    /// Templates archive (embedded or external)
    templates_data: Option<Vec<u8>>,
}

/// Template information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
    pub min_sdk: String,
    pub language: String,
}

/// Project configuration for template processing
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub package_name: String,
    pub min_sdk: String,
    pub target_sdk: String,
}

impl TemplateEngineRunner {
    /// Create new template engine runner
    pub fn new() -> Self {
        Self {
            templates_data: None,
        }
    }

    /// Get embedded templates data (minimal built-in templates)
    fn get_embedded_templates() -> Vec<u8> {
        // Create a minimal templates.zip in memory
        // This is a simplified embedded template structure
        vec![]
    }

    /// List available templates
    pub fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        // Built-in templates
        let templates = vec![
            TemplateInfo {
                name: "empty".to_string(),
                description: "Empty Android project with minimal setup".to_string(),
                min_sdk: "21".to_string(),
                language: "kotlin".to_string(),
            },
            TemplateInfo {
                name: "basic".to_string(),
                description: "Basic Android project with MainActivity".to_string(),
                min_sdk: "21".to_string(),
                language: "kotlin".to_string(),
            },
            TemplateInfo {
                name: "compose".to_string(),
                description: "Jetpack Compose project template".to_string(),
                min_sdk: "21".to_string(),
                language: "kotlin".to_string(),
            },
            TemplateInfo {
                name: "java-empty".to_string(),
                description: "Empty Android project in Java".to_string(),
                min_sdk: "21".to_string(),
                language: "java".to_string(),
            },
            TemplateInfo {
                name: "library".to_string(),
                description: "Android library project".to_string(),
                min_sdk: "21".to_string(),
                language: "kotlin".to_string(),
            },
        ];

        Ok(templates)
    }

    /// Print available templates
    pub fn print_templates(&self) -> Result<()> {
        let templates = self.list_templates()?;
        println!("Available templates:");
        println!();
        for template in &templates {
            println!("  {:15} {} (minSdk: {}, {})",
                template.name,
                template.description,
                template.min_sdk,
                template.language
            );
        }
        println!();
        println!("Usage: android create <template> --name <project-name>");
        Ok(())
    }

    /// Create project from template
    pub fn create_project(
        &self,
        template_name: &str,
        project_name: &str,
        output_dir: &Path,
        min_sdk: Option<&str>,
        verbose: bool,
    ) -> Result<()> {
        let templates = self.list_templates()?;

        let template = templates.iter()
            .find(|t| t.name == template_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Template '{}' not found. Run 'android create --list' to see available templates.",
                template_name
            ))?;

        let min_sdk = min_sdk.unwrap_or(&template.min_sdk);

        // Validate project name
        if !is_valid_project_name(project_name) {
            bail!(
                "Invalid project name '{}'. Use lowercase letters, numbers, and underscores only.",
                project_name
            );
        }

        // Generate package name from project name
        let package_name = generate_package_name(project_name);

        let config = ProjectConfig {
            name: project_name.to_string(),
            package_name: package_name.clone(),
            min_sdk: min_sdk.to_string(),
            target_sdk: "34".to_string(),
        };

        if verbose {
            println!("Creating project '{}' from template '{}'", project_name, template_name);
            println!("Package: {}", package_name);
            println!("Min SDK: {}", min_sdk);
            println!("Output: {}", output_dir.display());
        }

        // Create project directory
        let project_dir = output_dir.join(project_name);
        if project_dir.exists() {
            bail!(
                "Directory '{}' already exists. Please choose a different name or output directory.",
                project_dir.display()
            );
        }
        fs::create_dir_all(&project_dir)
            .with_context(|| format!("Failed to create project directory: {}", project_dir.display()))?;

        // Generate project structure based on template
        match template_name {
            "empty" | "basic" | "compose" | "java-empty" => {
                self.generate_app_project(&project_dir, &config, template_name, verbose)?;
            }
            "library" => {
                self.generate_library_project(&project_dir, &config, verbose)?;
            }
            _ => {
                // Generic template generation
                self.generate_app_project(&project_dir, &config, template_name, verbose)?;
            }
        }

        if verbose {
            println!();
            println!("Project created successfully!");
            println!("  cd {}", project_name);
            println!("  # Open in Android Studio or build with Gradle");
        } else {
            println!("Created project: {}", project_dir.display());
        }

        Ok(())
    }

    /// Generate Android application project
    fn generate_app_project(
        &self,
        project_dir: &Path,
        config: &ProjectConfig,
        template_name: &str,
        verbose: bool,
    ) -> Result<()> {
        let is_compose = template_name == "compose";
        let is_java = template_name == "java-empty";

        // Create directory structure
        let src_dir = if is_java {
            project_dir.join("app/src/main/java")
                .join(config.package_name.replace('.', "/"))
        } else {
            project_dir.join("app/src/main/java")
                .join(config.package_name.replace('.', "/"))
        };
        let res_dir = project_dir.join("app/src/main/res");
        let layout_dir = res_dir.join("layout");
        let values_dir = res_dir.join("values");
        let drawable_dir = res_dir.join("drawable");

        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&layout_dir)?;
        fs::create_dir_all(&values_dir)?;
        fs::create_dir_all(&drawable_dir)?;

        // Generate build.gradle.kts (project level)
        self.write_file(
            &project_dir.join("build.gradle.kts"),
            &generate_root_build_gradle(),
            verbose,
        )?;

        // Generate settings.gradle.kts
        self.write_file(
            &project_dir.join("settings.gradle.kts"),
            &generate_settings_gradle(&config.name),
            verbose,
        )?;

        // Generate app/build.gradle.kts
        self.write_file(
            &project_dir.join("app/build.gradle.kts"),
            &generate_app_build_gradle(config, is_compose, is_java),
            verbose,
        )?;

        // Generate AndroidManifest.xml
        self.write_file(
            &project_dir.join("app/src/main/AndroidManifest.xml"),
            &generate_android_manifest(config),
            verbose,
        )?;

        // Generate MainActivity
        if is_java {
            self.write_file(
                &src_dir.join("MainActivity.java"),
                &generate_main_activity_java(config),
                verbose,
            )?;
        } else if is_compose {
            self.write_file(
                &src_dir.join("MainActivity.kt"),
                &generate_main_activity_compose(config),
                verbose,
            )?;
            // Generate Compose theme
            let theme_dir = src_dir.join("ui/theme");
            fs::create_dir_all(&theme_dir)?;
            self.write_file(
                &theme_dir.join("Theme.kt"),
                &generate_compose_theme(config),
                verbose,
            )?;
        } else {
            self.write_file(
                &src_dir.join("MainActivity.kt"),
                &generate_main_activity_kotlin(config),
                verbose,
            )?;
            // Generate layout
            self.write_file(
                &layout_dir.join("activity_main.xml"),
                &generate_main_layout(config),
                verbose,
            )?;
        }

        // Generate strings.xml
        self.write_file(
            &values_dir.join("strings.xml"),
            &generate_strings_xml(&config.name),
            verbose,
        )?;

        // Generate colors.xml
        self.write_file(
            &values_dir.join("colors.xml"),
            &generate_colors_xml(),
            verbose,
        )?;

        // Generate themes.xml
        self.write_file(
            &values_dir.join("themes.xml"),
            &generate_themes_xml(config),
            verbose,
        )?;

        // Generate gradle.properties
        self.write_file(
            &project_dir.join("gradle.properties"),
            &generate_gradle_properties(),
            verbose,
        )?;

        // Generate .gitignore
        self.write_file(
            &project_dir.join(".gitignore"),
            &generate_gitignore(),
            verbose,
        )?;

        Ok(())
    }

    /// Generate Android library project
    fn generate_library_project(
        &self,
        project_dir: &Path,
        config: &ProjectConfig,
        verbose: bool,
    ) -> Result<()> {
        let src_dir = project_dir.join("lib/src/main/java")
            .join(config.package_name.replace('.', "/"));

        fs::create_dir_all(&src_dir)?;

        // Generate build.gradle.kts (project level)
        self.write_file(
            &project_dir.join("build.gradle.kts"),
            &generate_root_build_gradle(),
            verbose,
        )?;

        // Generate settings.gradle.kts
        self.write_file(
            &project_dir.join("settings.gradle.kts"),
            &generate_settings_gradle(&config.name),
            verbose,
        )?;

        // Generate lib/build.gradle.kts
        self.write_file(
            &project_dir.join("lib/build.gradle.kts"),
            &generate_lib_build_gradle(config),
            verbose,
        )?;

        // Generate AndroidManifest.xml
        self.write_file(
            &project_dir.join("lib/src/main/AndroidManifest.xml"),
            &generate_lib_android_manifest(config),
            verbose,
        )?;

        // Generate sample class
        self.write_file(
            &src_dir.join("LibraryClass.kt"),
            &generate_sample_library_class(config),
            verbose,
        )?;

        // Generate .gitignore
        self.write_file(
            &project_dir.join(".gitignore"),
            &generate_gitignore(),
            verbose,
        )?;

        Ok(())
    }

    /// Write file with optional verbose output
    fn write_file(&self, path: &Path, content: &str, verbose: bool) -> Result<()> {
        if verbose {
            println!("  Creating: {}", path.display());
        }
        fs::write(path, content)
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }
}

impl Default for TemplateEngineRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate project name
fn is_valid_project_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Must start with letter
    if !name.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false) {
        return false;
    }

    // Only lowercase letters, numbers, and underscores
    name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Generate package name from project name
fn generate_package_name(project_name: &str) -> String {
    format!("com.example.{}", project_name.replace('_', "").to_lowercase())
}

// Template generation functions

fn generate_root_build_gradle() -> String {
    r#"plugins {
    id("com.android.application") version "8.2.0" apply false
    id("org.jetbrains.kotlin.android") version "1.9.20" apply false
}
"#.to_string()
}

fn generate_settings_gradle(project_name: &str) -> String {
    format!(
        r#"pluginManagement {{
    repositories {{
        google()
        mavenCentral()
        gradlePluginPortal()
    }}
}}

dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {{
        google()
        mavenCentral()
    }}
}}

rootProject.name = "{}"
include(":app")
"#,
        project_name
    )
}

fn generate_app_build_gradle(config: &ProjectConfig, is_compose: bool, is_java: bool) -> String {
    let compose_deps = if is_compose {
        r#"
    // Compose
    implementation(platform("androidx.compose:compose-bom:2023.10.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    debugImplementation("androidx.compose.ui:ui-tooling")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
"#
    } else {
        ""
    };

    let kotlin_config = if is_java {
        ""
    } else {
        r#"
    kotlinOptions {
        jvmTarget = "17"
    }
"#
    };

    let compose_config = if is_compose {
        r#"
    buildFeatures {
        compose = true
    }
    composeOptions {
        kotlinCompilerExtensionVersion = "1.5.4"
    }
"#
    } else {
        ""
    };

    format!(
        r#"plugins {{
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}}

android {{
    namespace = "{}"
    compileSdk = 34

    defaultConfig {{
        applicationId = "{}"
        minSdk = {}
        targetSdk = 34
        versionCode = 1
        versionName = "1.0"
    }}
{}
    buildTypes {{
        release {{
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }}
    }}
    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }}
{}{}}}

dependencies {{
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("com.google.android.material:material:1.11.0")
    implementation("androidx.constraintlayout:constraintlayout:2.1.4"){}
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
}}
"#,
        config.package_name,
        config.package_name,
        config.min_sdk,
        kotlin_config,
        compose_config,
        compose_deps,
        if is_java { "    id(\"java\")" } else { "" }
    )
}

fn generate_android_manifest(config: &ProjectConfig) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">

    <application
        android:allowBackup="true"
        android:icon="@mipmap/ic_launcher"
        android:label="@string/app_name"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:supportsRtl="true"
        android:theme="@style/Theme.{}">
        <activity
            android:name=".MainActivity"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>

</manifest>
"#,
        config.name.replace('_', "").replace('-', "")
    )
}

fn generate_main_activity_kotlin(config: &ProjectConfig) -> String {
    format!(
        r#"package {}

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity

class MainActivity : AppCompatActivity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
    }}
}}
"#,
        config.package_name
    )
}

fn generate_main_activity_java(config: &ProjectConfig) -> String {
    format!(
        r#"package {};

import android.os.Bundle;
import androidx.appcompat.app.AppCompatActivity;

public class MainActivity extends AppCompatActivity {{
    @Override
    protected void onCreate(Bundle savedInstanceState) {{
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
    }}
}}
"#,
        config.package_name
    )
}

fn generate_main_layout(_config: &ProjectConfig) -> String {
    r#"<?xml version="1.0" encoding="utf-8"?>
<androidx.constraintlayout.widget.ConstraintLayout
    xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:app="http://schemas.android.com/apk/res-auto"
    xmlns:tools="http://schemas.android.com/tools"
    android:layout_width="match_parent"
    android:layout_height="match_parent"
    tools:context=".MainActivity">

    <TextView
        android:layout_width="wrap_content"
        android:layout_height="wrap_content"
        android:text="Hello World!"
        app:layout_constraintBottom_toBottomOf="parent"
        app:layout_constraintEnd_toEndOf="parent"
        app:layout_constraintStart_toStartOf="parent"
        app:layout_constraintTop_toTopOf="parent" />

</androidx.constraintlayout.widget.ConstraintLayout>
"#.to_string()
}

fn generate_main_activity_compose(config: &ProjectConfig) -> String {
    format!(
        r#"package {}

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import {}.ui.theme.Theme

class MainActivity : ComponentActivity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        setContent {{
            Theme {{
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {{
                    Greeting("Android")
                }}
            }}
        }}
    }}
}}

@Composable
fun Greeting(name: String, modifier: Modifier = Modifier) {{
    Text(
        text = "Hello $name!",
        modifier = modifier.padding(16.dp)
    )
}}
"#,
        config.package_name, config.package_name
    )
}

fn generate_compose_theme(config: &ProjectConfig) -> String {
    format!(
        r#"package {}.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val DarkColorScheme = darkColorScheme(
    primary = Color(0xFF6200EE),
    secondary = Color(0xFF03DAC6),
    tertiary = Color(0xFF3700B3)
)

private val LightColorScheme = lightColorScheme(
    primary = Color(0xFF6200EE),
    secondary = Color(0xFF03DAC6),
    tertiary = Color(0xFF3700B3)
)

@Composable
fun Theme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {{
    val colorScheme = if (darkTheme) {{
        DarkColorScheme
    }} else {{
        LightColorScheme
    }}

    MaterialTheme(
        colorScheme = colorScheme,
        content = content
    )
}}
"#,
        config.package_name
    )
}

fn generate_strings_xml(project_name: &str) -> String {
    format!(
        r#"<resources>
    <string name="app_name">{}</string>
</resources>
"#,
        project_name.replace('_', " ").replace('-', " ")
            .split_whitespace()
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn generate_colors_xml() -> String {
    r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="purple_200">#FFBB86FC</color>
    <color name="purple_500">#FF6200EE</color>
    <color name="purple_700">#FF3700B3</color>
    <color name="teal_200">#FF03DAC5</color>
    <color name="teal_700">#FF018786</color>
    <color name="black">#FF000000</color>
    <color name="white">#FFFFFFFF</color>
</resources>
"#.to_string()
}

fn generate_themes_xml(config: &ProjectConfig) -> String {
    format!(
        r#"<resources xmlns:tools="http://schemas.android.com/tools">
    <style name="Theme.{}" parent="Theme.MaterialComponents.DayNight.DarkActionBar">
        <item name="colorPrimary">@color/purple_500</item>
        <item name="colorPrimaryVariant">@color/purple_700</item>
        <item name="colorOnPrimary">@color/white</item>
        <item name="colorSecondary">@color/teal_200</item>
        <item name="colorSecondaryVariant">@color/teal_700</item>
        <item name="colorOnSecondary">@color/black</item>
    </style>
</resources>
"#,
        config.name.replace('_', "").replace('-', "")
    )
}

fn generate_gradle_properties() -> String {
    r#"# Project-wide Gradle settings.
org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
kotlin.code.style=official
android.nonTransitiveRClass=true
"#.to_string()
}

fn generate_gitignore() -> String {
    r#"# Built application files
*.apk
*.aar
*.ap_
*.aab

# Files for the ART/Dalvik VM
*.dex

# Java class files
*.class

# Generated files
bin/
gen/
out/
release/

# Gradle files
.gradle/
build/

# Local configuration file (sdk path, etc)
local.properties

# Proguard folder generated by Eclipse
proguard/

# Log Files
*.log

# Android Studio Navigation editor temp files
.navigation/

# Android Studio captures folder
captures/

# IntelliJ
*.iml
.idea/
.idea/workspace.xml
.idea/tasks.xml
.idea/gradle.xml
.idea/assetWizardSettings.xml
.idea/dictionaries
.idea/libraries
.idea/caches

# Keystore files
*.jks
*.keystore

# External native build folder generated in Android Studio 2.2 and later
.externalNativeBuild
.cxx/

# Google Services (e.g. APIs or Firebase)
google-services.json

# Freeline
freeline.py
freeline/
freeline_project_description.json

# fastlane
fastlane/report.xml
fastlane/Preview.html
fastlane/screenshots
fastlane/test_output
fastlane/readme.md

# Version control
vcs.xml

# lint
lint/intermediates/
lint/generated/
lint/outputs/
lint/tmp/
lint/reports/

# OS generated files
.DS_Store
.DS_Store?
._*
.Spotlight-V100
.Trashes
ehthumbs.db
Thumbs.db
"#.to_string()
}

fn generate_lib_build_gradle(config: &ProjectConfig) -> String {
    format!(
        r#"plugins {{
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}}

android {{
    namespace = "{}"
    compileSdk = 34

    defaultConfig {{
        minSdk = {}
        targetSdk = 34
    }}

    buildTypes {{
        release {{
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }}
    }}
    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }}
    kotlinOptions {{
        jvmTarget = "17"
    }}
}}

dependencies {{
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("com.google.android.material:material:1.11.0")
}}
"#,
        config.package_name, config.min_sdk
    )
}

fn generate_lib_android_manifest(config: &ProjectConfig) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <!-- Add library-specific configuration here -->
</manifest>
"#
    )
}

fn generate_sample_library_class(config: &ProjectConfig) -> String {
    format!(
        r#"package {}

/**
 * Sample library class
 */
class LibraryClass {{
    fun doSomething(): String {{
        return "Hello from {}"
    }}
}}
"#,
        config.package_name, config.package_name
    )
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
        assert!(templates.iter().any(|t| t.name == "basic"));
    }

    #[test]
    fn test_valid_project_names() {
        assert!(is_valid_project_name("myapp"));
        assert!(is_valid_project_name("my_app"));
        assert!(is_valid_project_name("app123"));
        assert!(is_valid_project_name("myapp123"));
    }

    #[test]
    fn test_invalid_project_names() {
        assert!(!is_valid_project_name(""));
        assert!(!is_valid_project_name("1app"));
        assert!(!is_valid_project_name("MyApp"));
        assert!(!is_valid_project_name("my-app"));
        assert!(!is_valid_project_name("my app"));
    }

    #[test]
    fn test_generate_package_name() {
        assert_eq!(generate_package_name("myapp"), "com.example.myapp");
        assert_eq!(generate_package_name("my_app"), "com.example.myapp");
    }

    #[test]
    fn test_create_project() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("basic", "testapp", dir.path(), None, false).unwrap();

        assert!(dir.path().join("testapp").exists());
        assert!(dir.path().join("testapp/build.gradle.kts").exists());
        assert!(dir.path().join("testapp/app/build.gradle.kts").exists());
    }

    #[test]
    fn test_template_info_creation() {
        let template = TemplateInfo {
            name: "test_template".to_string(),
            description: "A test template".to_string(),
            min_sdk: "21".to_string(),
            language: "kotlin".to_string(),
        };

        assert_eq!(template.name, "test_template");
        assert_eq!(template.description, "A test template");
        assert_eq!(template.min_sdk, "21");
        assert_eq!(template.language, "kotlin");
    }

    #[test]
    fn test_template_info_serialization() {
        let template = TemplateInfo {
            name: "basic".to_string(),
            description: "Basic template".to_string(),
            min_sdk: "23".to_string(),
            language: "java".to_string(),
        };

        let json = serde_json::to_string(&template).unwrap();
        let deserialized: TemplateInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, template.name);
        assert_eq!(deserialized.description, template.description);
        assert_eq!(deserialized.min_sdk, template.min_sdk);
        assert_eq!(deserialized.language, template.language);
    }

    #[test]
    fn test_project_config_validation() {
        // Test valid project name validation
        assert!(is_valid_project_name("myapp"));
        assert!(is_valid_project_name("my_app_123"));
        assert!(is_valid_project_name("testapp"));

        // Test invalid project name validation
        assert!(!is_valid_project_name(""));
        assert!(!is_valid_project_name("1app")); // starts with digit
        assert!(!is_valid_project_name("MyApp")); // has uppercase
        assert!(!is_valid_project_name("my-app")); // has hyphen
        assert!(!is_valid_project_name("my app")); // has space
        assert!(!is_valid_project_name("_app")); // starts with underscore
    }

    #[test]
    fn test_project_config_package_name_generation() {
        assert_eq!(generate_package_name("myapp"), "com.example.myapp");
        assert_eq!(generate_package_name("my_app"), "com.example.myapp");
        assert_eq!(generate_package_name("test123"), "com.example.test123");
        assert_eq!(generate_package_name("a"), "com.example.a");
    }

    #[test]
    fn test_generate_gradle_files() {
        let config = ProjectConfig {
            name: "test_app".to_string(),
            package_name: "com.example.testapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        // Test root build.gradle.kts
        let root_gradle = generate_root_build_gradle();
        assert!(root_gradle.contains("com.android.application"));
        assert!(root_gradle.contains("org.jetbrains.kotlin.android"));

        // Test settings.gradle.kts
        let settings_gradle = generate_settings_gradle("test_app");
        assert!(settings_gradle.contains("test_app"));
        assert!(settings_gradle.contains("include(\":app\")"));

        // Test app build.gradle.kts
        let app_gradle = generate_app_build_gradle(&config, false, false);
        assert!(app_gradle.contains("com.example.testapp"));
        assert!(app_gradle.contains("minSdk = 21"));
        assert!(app_gradle.contains("targetSdk = 34"));
        assert!(app_gradle.contains("kotlinOptions"));

        // Test Java variant
        let java_gradle = generate_app_build_gradle(&config, false, true);
        assert!(java_gradle.contains("id(\"java\")"));

        // Test Compose variant
        let compose_gradle = generate_app_build_gradle(&config, true, false);
        assert!(compose_gradle.contains("compose-bom"));
        assert!(compose_gradle.contains("compose = true"));
    }

    #[test]
    fn test_generate_manifest() {
        let config = ProjectConfig {
            name: "test_app".to_string(),
            package_name: "com.example.testapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let manifest = generate_android_manifest(&config);

        // Check manifest structure
        assert!(manifest.contains("<?xml version=\"1.0\""));
        assert!(manifest.contains("<manifest"));
        assert!(manifest.contains("<application"));
        assert!(manifest.contains("<activity"));
        assert!(manifest.contains("android:name=\".MainActivity\""));
        assert!(manifest.contains("android.intent.action.MAIN"));
        assert!(manifest.contains("android.intent.category.LAUNCHER"));
        // Theme name becomes lowercase after removing underscores
        assert!(manifest.contains("Theme.testapp"));
    }

    #[test]
    fn test_generate_kotlin_activity() {
        let config = ProjectConfig {
            name: "test_app".to_string(),
            package_name: "com.example.testapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let activity = generate_main_activity_kotlin(&config);

        assert!(activity.contains("package com.example.testapp"));
        assert!(activity.contains("import android.os.Bundle"));
        assert!(activity.contains("import androidx.appcompat.app.AppCompatActivity"));
        assert!(activity.contains("class MainActivity : AppCompatActivity()"));
        // Kotlin uses nullable Bundle (Bundle?)
        assert!(activity.contains("onCreate(savedInstanceState: Bundle?)"));
        assert!(activity.contains("super.onCreate(savedInstanceState)"));
        assert!(activity.contains("setContentView(R.layout.activity_main)"));
    }

    #[test]
    fn test_generate_java_activity() {
        let config = ProjectConfig {
            name: "test_app".to_string(),
            package_name: "com.example.testapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let activity = generate_main_activity_java(&config);

        assert!(activity.contains("package com.example.testapp;"));
        assert!(activity.contains("import android.os.Bundle;"));
        assert!(activity.contains("import androidx.appcompat.app.AppCompatActivity;"));
        assert!(activity.contains("public class MainActivity extends AppCompatActivity"));
        assert!(activity.contains("protected void onCreate(Bundle savedInstanceState)"));
        assert!(activity.contains("setContentView(R.layout.activity_main);"));
    }

    #[test]
    fn test_generate_compose_activity() {
        let config = ProjectConfig {
            name: "compose_app".to_string(),
            package_name: "com.example.composeapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let activity = generate_main_activity_compose(&config);

        assert!(activity.contains("package com.example.composeapp"));
        assert!(activity.contains("import androidx.activity.ComponentActivity"));
        assert!(activity.contains("import androidx.activity.compose.setContent"));
        assert!(activity.contains("import androidx.compose"));
        assert!(activity.contains("class MainActivity : ComponentActivity()"));
        assert!(activity.contains("@Composable"));
        assert!(activity.contains("fun Greeting"));
        assert!(activity.contains("com.example.composeapp.ui.theme.Theme"));
    }

    #[test]
    fn test_template_list() {
        let runner = TemplateEngineRunner::new();
        let templates = runner.list_templates().unwrap();

        // Check that all expected templates are present
        let template_names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
        assert!(template_names.contains(&"empty"));
        assert!(template_names.contains(&"basic"));
        assert!(template_names.contains(&"compose"));
        assert!(template_names.contains(&"java-empty"));
        assert!(template_names.contains(&"library"));

        // Verify template properties
        let basic_template = templates.iter().find(|t| t.name == "basic").unwrap();
        assert_eq!(basic_template.min_sdk, "21");
        assert_eq!(basic_template.language, "kotlin");

        let java_template = templates.iter().find(|t| t.name == "java-empty").unwrap();
        assert_eq!(java_template.language, "java");

        let compose_template = templates.iter().find(|t| t.name == "compose").unwrap();
        assert!(compose_template.description.contains("Compose"));
    }

    #[test]
    fn test_generate_strings_xml() {
        let strings = generate_strings_xml("test_app");
        assert!(strings.contains("<string name=\"app_name\">"));
        assert!(strings.contains("</resources>"));

        let strings_with_underscores = generate_strings_xml("my_awesome_app");
        assert!(strings_with_underscores.contains("<string name=\"app_name\">"));
    }

    #[test]
    fn test_generate_colors_xml() {
        let colors = generate_colors_xml();
        assert!(colors.contains("purple_200"));
        assert!(colors.contains("purple_500"));
        assert!(colors.contains("purple_700"));
        assert!(colors.contains("teal_200"));
        assert!(colors.contains("black"));
        assert!(colors.contains("white"));
    }

    #[test]
    fn test_generate_themes_xml() {
        let config = ProjectConfig {
            name: "test_app".to_string(),
            package_name: "com.example.testapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let themes = generate_themes_xml(&config);
        // Theme name becomes lowercase after removing underscores
        assert!(themes.contains("Theme.testapp"));
        assert!(themes.contains("colorPrimary"));
        assert!(themes.contains("colorSecondary"));
    }

    #[test]
    fn test_generate_main_layout() {
        let config = ProjectConfig {
            name: "test_app".to_string(),
            package_name: "com.example.testapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let layout = generate_main_layout(&config);
        assert!(layout.contains("ConstraintLayout"));
        assert!(layout.contains("TextView"));
        assert!(layout.contains("Hello World!"));
    }

    #[test]
    fn test_generate_gradle_properties() {
        let props = generate_gradle_properties();
        assert!(props.contains("org.gradle.jvmargs"));
        assert!(props.contains("android.useAndroidX=true"));
        assert!(props.contains("kotlin.code.style=official"));
    }

    #[test]
    fn test_generate_gitignore() {
        let gitignore = generate_gitignore();
        assert!(gitignore.contains("*.apk"));
        assert!(gitignore.contains("build/"));
        assert!(gitignore.contains(".gradle/"));
        assert!(gitignore.contains(".idea/"));
        assert!(gitignore.contains("local.properties"));
    }

    #[test]
    fn test_create_empty_template() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("empty", "emptyapp", dir.path(), None, false).unwrap();

        assert!(dir.path().join("emptyapp/build.gradle.kts").exists());
        assert!(dir.path().join("emptyapp/app/src/main/java/com/example/emptyapp/MainActivity.kt").exists());
    }

    #[test]
    fn test_create_compose_template() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("compose", "composeapp", dir.path(), None, false).unwrap();

        assert!(dir.path().join("composeapp/build.gradle.kts").exists());
        assert!(dir.path().join("composeapp/app/src/main/java/com/example/composeapp/MainActivity.kt").exists());
        assert!(dir.path().join("composeapp/app/src/main/java/com/example/composeapp/ui/theme/Theme.kt").exists());
    }

    #[test]
    fn test_create_java_template() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("java-empty", "javaapp", dir.path(), None, false).unwrap();

        assert!(dir.path().join("javaapp/build.gradle.kts").exists());
        assert!(dir.path().join("javaapp/app/src/main/java/com/example/javaapp/MainActivity.java").exists());
    }

    #[test]
    fn test_create_library_template() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("library", "mylib", dir.path(), None, false).unwrap();

        assert!(dir.path().join("mylib/build.gradle.kts").exists());
        assert!(dir.path().join("mylib/lib/build.gradle.kts").exists());
        assert!(dir.path().join("mylib/lib/src/main/java/com/example/mylib/LibraryClass.kt").exists());
    }

    #[test]
    fn test_create_project_with_custom_min_sdk() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("basic", "sdkapp", dir.path(), Some("23"), false).unwrap();

        let build_gradle = std::fs::read_to_string(dir.path().join("sdkapp/app/build.gradle.kts")).unwrap();
        assert!(build_gradle.contains("minSdk = 23"));
    }

    #[test]
    fn test_create_project_verbose_output() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        runner.create_project("basic", "verboseapp", dir.path(), None, true).unwrap();

        assert!(dir.path().join("verboseapp").exists());
    }

    #[test]
    fn test_create_project_duplicate_directory() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        // Create first project
        runner.create_project("basic", "dupapp", dir.path(), None, false).unwrap();

        // Attempt to create same project again should fail
        let result = runner.create_project("basic", "dupapp", dir.path(), None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_project_invalid_name() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        // Test with invalid project name
        let result = runner.create_project("basic", "1InvalidName", dir.path(), None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_project_invalid_template() {
        let runner = TemplateEngineRunner::new();
        let dir = tempdir().unwrap();

        // Test with non-existent template
        let result = runner.create_project("nonexistent", "myapp", dir.path(), None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_lib_build_gradle() {
        let config = ProjectConfig {
            name: "mylib".to_string(),
            package_name: "com.example.mylib".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let lib_gradle = generate_lib_build_gradle(&config);
        assert!(lib_gradle.contains("com.android.library"));
        assert!(lib_gradle.contains("com.example.mylib"));
        assert!(lib_gradle.contains("minSdk = 21"));
    }

    #[test]
    fn test_generate_lib_android_manifest() {
        let config = ProjectConfig {
            name: "mylib".to_string(),
            package_name: "com.example.mylib".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let manifest = generate_lib_android_manifest(&config);
        assert!(manifest.contains("<manifest"));
    }

    #[test]
    fn test_generate_sample_library_class() {
        let config = ProjectConfig {
            name: "mylib".to_string(),
            package_name: "com.example.mylib".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let class = generate_sample_library_class(&config);
        assert!(class.contains("package com.example.mylib"));
        assert!(class.contains("class LibraryClass"));
        assert!(class.contains("fun doSomething()"));
    }

    #[test]
    fn test_generate_compose_theme() {
        let config = ProjectConfig {
            name: "composeapp".to_string(),
            package_name: "com.example.composeapp".to_string(),
            min_sdk: "21".to_string(),
            target_sdk: "34".to_string(),
        };

        let theme = generate_compose_theme(&config);
        assert!(theme.contains("package com.example.composeapp.ui.theme"));
        assert!(theme.contains("@Composable"));
        assert!(theme.contains("fun Theme"));
        assert!(theme.contains("MaterialTheme"));
        assert!(theme.contains("DarkColorScheme"));
        assert!(theme.contains("LightColorScheme"));
    }
}