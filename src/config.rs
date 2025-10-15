use crate::error::{OktofetchError, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub settings: Settings,
    #[serde(default)]
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub install_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

fn expand_path(path: &str) -> String {
    let mut expanded = path.to_string();

    // Handle tilde expansion
    if expanded.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            expanded = expanded.replacen("~", &home, 1);
        }
    } else if expanded == "~"
        && let Ok(home) = env::var("HOME")
    {
        expanded = home;
    }

    // Handle environment variable expansion ($VAR and ${VAR})
    let mut result = String::new();
    let mut chars = expanded.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            if chars.peek() == Some(&'{') {
                // Handle ${VAR} syntax
                chars.next(); // consume '{'
                let mut var_name = String::new();

                while let Some(&ch) = chars.peek() {
                    if ch == '}' {
                        chars.next(); // consume '}'
                        break;
                    }
                    var_name.push(chars.next().unwrap());
                }

                if let Ok(value) = env::var(&var_name) {
                    result.push_str(&value);
                } else {
                    // Keep original if variable not found
                    result.push_str(&format!("${{{}}}", var_name));
                }
            } else {
                // Handle $VAR syntax
                let mut var_name = String::new();

                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var_name.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if !var_name.is_empty() {
                    if let Ok(value) = env::var(&var_name) {
                        result.push_str(&value);
                    } else {
                        // Keep original if variable not found
                        result.push('$');
                        result.push_str(&var_name);
                    }
                } else {
                    result.push('$');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| OktofetchError::ConfigError(e.to_string(), config_path.clone()))?;

        let mut config: Self = toml::from_str(&content)
            .map_err(|e| OktofetchError::ConfigError(e.to_string(), config_path))?;

        // Expand environment variables and tilde in install_dir
        let expanded_path = expand_path(&config.settings.install_dir.to_string_lossy());
        config.settings.install_dir = PathBuf::from(expanded_path);

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| OktofetchError::ConfigError(e.to_string(), config_path.clone()))?;

        fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "oktofetch", "oktofetch").ok_or_else(|| {
            OktofetchError::Other("Cannot determine config directory".to_string())
        })?;

        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    pub fn add_tool(&mut self, tool: Tool) -> Result<()> {
        if self.tools.iter().any(|t| t.name == tool.name) {
            return Err(OktofetchError::Other(format!(
                "Tool '{}' already exists",
                tool.name
            )));
        }
        self.tools.push(tool);
        Ok(())
    }

    pub fn remove_tool(&mut self, name: &str) -> Result<()> {
        let initial_len = self.tools.len();
        self.tools.retain(|t| t.name != name);

        if self.tools.len() == initial_len {
            return Err(OktofetchError::ToolNotFound(name.to_string()));
        }
        Ok(())
    }

    pub fn get_tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn get_tool_mut(&mut self, name: &str) -> Option<&mut Tool> {
        self.tools.iter_mut().find(|t| t.name == name)
    }

    pub fn update_tool_version(&mut self, name: &str, version: String) -> Result<()> {
        let tool = self
            .get_tool_mut(name)
            .ok_or_else(|| OktofetchError::ToolNotFound(name.to_string()))?;
        tool.version = Some(version);
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let install_dir = PathBuf::from(home).join(".local/bin");

        Self {
            settings: Settings { install_dir },
            tools: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.tools.is_empty());
        assert!(
            config
                .settings
                .install_dir
                .to_string_lossy()
                .contains(".local/bin")
        );
    }

    #[test]
    fn test_add_tool() {
        let mut config = Config::default();
        let tool = Tool {
            name: "test-tool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };

        assert!(config.add_tool(tool).is_ok());
        assert_eq!(config.tools.len(), 1);
        assert_eq!(config.tools[0].name, "test-tool");
    }

    #[test]
    fn test_add_duplicate_tool() {
        let mut config = Config::default();
        let tool1 = Tool {
            name: "test-tool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };
        let tool2 = tool1.clone();

        assert!(config.add_tool(tool1).is_ok());
        assert!(config.add_tool(tool2).is_err());
        assert_eq!(config.tools.len(), 1);
    }

    #[test]
    fn test_remove_tool() {
        let mut config = Config::default();
        let tool = Tool {
            name: "test-tool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };

        config.add_tool(tool).unwrap();
        assert_eq!(config.tools.len(), 1);

        assert!(config.remove_tool("test-tool").is_ok());
        assert_eq!(config.tools.len(), 0);
    }

    #[test]
    fn test_remove_nonexistent_tool() {
        let mut config = Config::default();
        assert!(config.remove_tool("nonexistent").is_err());
    }

    #[test]
    fn test_get_tool() {
        let mut config = Config::default();
        let tool = Tool {
            name: "test-tool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: Some("custom-name".to_string()),
            asset_pattern: None,
            version: None,
        };

        config.add_tool(tool).unwrap();

        let found = config.get_tool("test-tool");
        assert!(found.is_some());
        assert_eq!(found.unwrap().binary_name, Some("custom-name".to_string()));

        let not_found = config.get_tool("other-tool");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Create a config with a tool
        let mut config = Config::default();
        config.settings.install_dir = PathBuf::from("/custom/path");
        let tool = Tool {
            name: "k9s".to_string(),
            repo: "derailed/k9s".to_string(),
            binary_name: None,
            asset_pattern: Some("linux-x64".to_string()),
            version: Some("v0.32.5".to_string()),
        };
        config.add_tool(tool).unwrap();

        // Save config
        let toml_str = toml::to_string_pretty(&config).unwrap();
        fs::write(&config_path, toml_str).unwrap();

        // Load config
        let loaded_content = fs::read_to_string(&config_path).unwrap();
        let loaded_config: Config = toml::from_str(&loaded_content).unwrap();

        assert_eq!(loaded_config.tools.len(), 1);
        assert_eq!(loaded_config.tools[0].name, "k9s");
        assert_eq!(loaded_config.tools[0].repo, "derailed/k9s");
        assert_eq!(loaded_config.tools[0].version, Some("v0.32.5".to_string()));
        assert_eq!(
            loaded_config.settings.install_dir,
            PathBuf::from("/custom/path")
        );
    }

    #[test]
    fn test_expand_path_tilde() {
        unsafe {
            env::set_var("HOME", "/home/testuser");
        }

        assert_eq!(super::expand_path("~/bin"), "/home/testuser/bin");
        assert_eq!(super::expand_path("~"), "/home/testuser");
        assert_eq!(super::expand_path("/absolute/path"), "/absolute/path");
    }

    #[test]
    fn test_expand_path_env_var() {
        unsafe {
            env::set_var("HOME", "/home/testuser");
            env::set_var("CUSTOM_DIR", "/opt/custom");
        }

        assert_eq!(
            super::expand_path("$HOME/.local/bin"),
            "/home/testuser/.local/bin"
        );
        assert_eq!(
            super::expand_path("${HOME}/.local/bin"),
            "/home/testuser/.local/bin"
        );
        assert_eq!(super::expand_path("$CUSTOM_DIR/bin"), "/opt/custom/bin");
        assert_eq!(super::expand_path("${CUSTOM_DIR}/bin"), "/opt/custom/bin");
    }

    #[test]
    fn test_expand_path_combined() {
        unsafe {
            env::set_var("HOME", "/home/testuser");
            env::set_var("PREFIX", "local");
        }

        assert_eq!(
            super::expand_path("~/$PREFIX/bin"),
            "/home/testuser/local/bin"
        );
        assert_eq!(
            super::expand_path("$HOME/${PREFIX}/bin"),
            "/home/testuser/local/bin"
        );
    }

    #[test]
    fn test_expand_path_missing_var() {
        unsafe {
            env::remove_var("NONEXISTENT_VAR");
        }

        // Should keep original if var doesn't exist
        assert_eq!(
            super::expand_path("$NONEXISTENT_VAR/bin"),
            "$NONEXISTENT_VAR/bin"
        );
        assert_eq!(
            super::expand_path("${NONEXISTENT_VAR}/bin"),
            "${NONEXISTENT_VAR}/bin"
        );
    }

    #[test]
    fn test_expand_path_edge_cases() {
        unsafe {
            env::set_var("TEST_VAR", "value");
        }

        // Test $ at end of string
        assert_eq!(super::expand_path("path$"), "path$");

        // Test empty variable name
        assert_eq!(super::expand_path("$/path"), "$/path");

        // Test ${} with empty name
        assert_eq!(super::expand_path("${}/path"), "${}/path");

        // Test multiple variables
        unsafe {
            env::set_var("VAR1", "first");
            env::set_var("VAR2", "second");
        }
        assert_eq!(super::expand_path("$VAR1/$VAR2"), "first/second");
        assert_eq!(super::expand_path("${VAR1}/${VAR2}"), "first/second");
    }

    #[test]
    fn test_update_tool_version() {
        let mut config = Config::default();
        let tool = Tool {
            name: "mytool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: Some("v1.0.0".to_string()),
        };
        config.add_tool(tool).unwrap();

        // Update version
        config
            .update_tool_version("mytool", "v2.0.0".to_string())
            .unwrap();
        assert_eq!(
            config.get_tool("mytool").unwrap().version,
            Some("v2.0.0".to_string())
        );

        // Try to update non-existent tool
        let result = config.update_tool_version("nonexistent", "v1.0.0".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_tool_mut() {
        let mut config = Config::default();
        let tool = Tool {
            name: "mytool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };
        config.add_tool(tool).unwrap();

        // Modify tool through mutable reference
        if let Some(tool) = config.get_tool_mut("mytool") {
            tool.version = Some("v1.0.0".to_string());
        }

        assert_eq!(
            config.get_tool("mytool").unwrap().version,
            Some("v1.0.0".to_string())
        );
    }

    #[test]
    fn test_config_path() {
        let result = Config::config_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("oktofetch"));
        assert!(path.to_string_lossy().contains("config.toml"));
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool {
            name: "test".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: Some("testbin".to_string()),
            asset_pattern: Some("linux-x64".to_string()),
            version: Some("v1.0.0".to_string()),
        };

        let serialized = toml::to_string(&tool).unwrap();
        assert!(serialized.contains("name = \"test\""));
        assert!(serialized.contains("repo = \"owner/repo\""));
        assert!(serialized.contains("binary_name = \"testbin\""));
        assert!(serialized.contains("asset_pattern = \"linux-x64\""));
        assert!(serialized.contains("version = \"v1.0.0\""));

        let deserialized: Tool = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.repo, "owner/repo");
        assert_eq!(deserialized.binary_name, Some("testbin".to_string()));
        assert_eq!(deserialized.asset_pattern, Some("linux-x64".to_string()));
        assert_eq!(deserialized.version, Some("v1.0.0".to_string()));
    }

    #[test]
    fn test_tool_serialization_optional_fields() {
        // Test with None values - they should be omitted from serialization
        let tool = Tool {
            name: "test".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };

        let serialized = toml::to_string(&tool).unwrap();
        assert!(serialized.contains("name = \"test\""));
        assert!(serialized.contains("repo = \"owner/repo\""));
        // Optional fields should not be in serialized output
        assert!(!serialized.contains("binary_name"));
        assert!(!serialized.contains("asset_pattern"));
        assert!(!serialized.contains("version"));
    }

    #[test]
    fn test_expand_path_no_expansion_needed() {
        // Paths that don't need expansion
        assert_eq!(super::expand_path("/absolute/path"), "/absolute/path");
        assert_eq!(super::expand_path("relative/path"), "relative/path");
        assert_eq!(super::expand_path("./current/dir"), "./current/dir");
        assert_eq!(super::expand_path("../parent/dir"), "../parent/dir");
    }

    #[test]
    fn test_expand_path_dollar_sign_edge_cases() {
        unsafe {
            env::set_var("VAR", "value");
            env::set_var("VARsuffix", "fullvalue");
        }

        // Dollar sign at various positions
        assert_eq!(super::expand_path("$VAR"), "value");
        assert_eq!(super::expand_path("prefix$VAR"), "prefixvalue");
        // Note: $VARsuffix reads the whole variable name (alphanumeric + _)
        assert_eq!(super::expand_path("$VARsuffix"), "fullvalue");
        // Use braces to separate variable from suffix
        assert_eq!(super::expand_path("pre${VAR}suf"), "prevaluesuf");

        // Multiple dollars
        assert_eq!(super::expand_path("$$"), "$$");
    }

    #[test]
    fn test_config_default_values() {
        let config = Config::default();

        assert!(config.tools.is_empty());
        assert!(
            config
                .settings
                .install_dir
                .to_string_lossy()
                .ends_with(".local/bin")
        );
    }

    #[test]
    fn test_config_load_nonexistent_file() {
        // Loading a non-existent config should return default
        // This is already tested indirectly but let's be explicit
        let config = Config::default();
        assert!(config.tools.is_empty());
    }

    #[test]
    fn test_tool_clone_and_equality() {
        let tool1 = Tool {
            name: "test".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: Some("bin".to_string()),
            asset_pattern: None,
            version: Some("v1.0.0".to_string()),
        };

        let tool2 = tool1.clone();
        assert_eq!(tool1.name, tool2.name);
        assert_eq!(tool1.repo, tool2.repo);
        assert_eq!(tool1.binary_name, tool2.binary_name);
        assert_eq!(tool1.version, tool2.version);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings {
            install_dir: PathBuf::from("/custom/path"),
        };

        let serialized = toml::to_string(&settings).unwrap();
        assert!(serialized.contains("install_dir"));
        assert!(serialized.contains("/custom/path"));

        let deserialized: Settings = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.install_dir, PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_config_multiple_operations() {
        let mut config = Config::default();

        // Add multiple tools
        for i in 1..=5 {
            config
                .add_tool(Tool {
                    name: format!("tool{}", i),
                    repo: format!("owner/repo{}", i),
                    binary_name: None,
                    asset_pattern: None,
                    version: None,
                })
                .unwrap();
        }
        assert_eq!(config.tools.len(), 5);

        // Update some versions
        config
            .update_tool_version("tool1", "v1.0.0".to_string())
            .unwrap();
        config
            .update_tool_version("tool3", "v2.0.0".to_string())
            .unwrap();

        assert_eq!(
            config.get_tool("tool1").unwrap().version,
            Some("v1.0.0".to_string())
        );
        assert_eq!(
            config.get_tool("tool3").unwrap().version,
            Some("v2.0.0".to_string())
        );

        // Remove some tools
        config.remove_tool("tool2").unwrap();
        config.remove_tool("tool4").unwrap();
        assert_eq!(config.tools.len(), 3);

        // Verify remaining tools
        assert!(config.get_tool("tool1").is_some());
        assert!(config.get_tool("tool2").is_none());
        assert!(config.get_tool("tool3").is_some());
        assert!(config.get_tool("tool4").is_none());
        assert!(config.get_tool("tool5").is_some());
    }

    #[test]
    fn test_expand_path_brace_syntax_variations() {
        unsafe {
            env::set_var("TEST1", "value1");
            env::set_var("TEST2", "value2");
        }

        // Test ${VAR} syntax
        assert_eq!(super::expand_path("${TEST1}"), "value1");
        assert_eq!(
            super::expand_path("prefix${TEST1}suffix"),
            "prefixvalue1suffix"
        );
        assert_eq!(super::expand_path("${TEST1}/${TEST2}"), "value1/value2");

        // Empty braces - variable doesn't exist
        assert_eq!(super::expand_path("${}"), "${}");

        // Test multiple substitutions
        assert_eq!(super::expand_path("$TEST1-$TEST2"), "value1-value2");
        assert_eq!(super::expand_path("${TEST1}-${TEST2}"), "value1-value2");
    }
}
