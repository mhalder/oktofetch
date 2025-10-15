use crate::archive;
use crate::binary;
use crate::config::{Config, Tool};
use crate::error::{OktofetchError, Result};
use crate::github::GithubClient;
use crate::platform;
use tempfile::TempDir;

pub async fn add_tool(
    config: &mut Config,
    repo: String,
    name: Option<String>,
    binary_name: Option<String>,
) -> Result<()> {
    let repo = parse_repo(&repo)?;
    let tool_name = name.unwrap_or_else(|| {
        binary_name
            .clone()
            .unwrap_or_else(|| repo.split('/').next_back().unwrap_or(&repo).to_string())
    });

    let tool = Tool {
        name: tool_name.clone(),
        repo: repo.clone(),
        binary_name,
        asset_pattern: None,
        version: None,
    };

    config.add_tool(tool)?;
    config.save()?;
    println!("Added tool '{}' ({})", tool_name, repo);
    Ok(())
}

fn asset_priority(name: &str) -> u8 {
    let name = name.to_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        0 // Highest priority
    } else if name.ends_with(".zip") {
        1 // Second priority
    } else {
        2 // Lowest priority
    }
}

pub async fn update_tool(
    config: &mut Config,
    tool_name: &str,
    verbose: bool,
    force: bool,
) -> Result<()> {
    let tool = config
        .get_tool(tool_name)
        .ok_or_else(|| OktofetchError::ToolNotFound(tool_name.to_string()))?
        .clone();

    if verbose {
        println!("Updating {} from {}", tool.name, tool.repo);
    }

    // Show current version if available
    if let Some(current_version) = &tool.version {
        println!("Current version: {}", current_version);
    } else {
        println!("Current version: unknown");
    }

    // Validate platform
    platform::validate_platform()?;

    // Fetch latest release
    let client = GithubClient::new();
    let release = client.get_latest_release(&tool.repo).await?;

    println!("Latest version: {}", release.tag_name);

    // Check if binary exists on disk
    let binary_name = tool.binary_name.as_deref().unwrap_or(&tool.name);
    let binary_path = config.settings.install_dir.join(binary_name);
    let binary_exists = binary_path.exists();

    if !binary_exists {
        println!(
            "Binary not found at {}, reinstalling...",
            binary_path.display()
        );
    }

    // Check if update is needed
    if !force
        && binary_exists
        && let Some(current_version) = &tool.version
        && current_version == &release.tag_name
    {
        println!("{} is already up to date", tool.name);
        return Ok(());
    }

    if verbose {
        println!("Found release: {}", release.tag_name);
    }

    // Find matching asset
    let asset = if let Some(pattern) = &tool.asset_pattern {
        release
            .assets
            .iter()
            .find(|a| a.name.contains(pattern))
            .ok_or_else(|| OktofetchError::NoSuitableRelease {
                platform: "Linux".to_string(),
                arch: "x86_64".to_string(),
            })?
    } else {
        // Filter assets matching the platform
        let mut matching_assets: Vec<_> = release
            .assets
            .iter()
            .filter(|a| platform::matches_asset_name(&a.name))
            .collect();

        if matching_assets.is_empty() {
            return Err(OktofetchError::NoSuitableRelease {
                platform: "Linux".to_string(),
                arch: "x86_64".to_string(),
            });
        }

        // Sort by priority: tar.gz/tgz first, then zip, then others
        matching_assets.sort_by_key(|a| asset_priority(&a.name));

        matching_assets[0]
    };

    if verbose {
        println!("Selected asset: {}", asset.name);
    }

    // Download to temp directory
    let temp_dir = TempDir::new()?;
    let archive_path = temp_dir.path().join(&asset.name);

    println!("Downloading {}...", asset.name);
    client
        .download_asset(&asset.browser_download_url, &archive_path)
        .await?;

    // Extract archive
    if verbose {
        println!("Extracting archive...");
    }
    let extracted_files = archive::extract_archive(&archive_path, temp_dir.path())?;

    // Find binary
    let binary_name = tool.binary_name.as_deref().unwrap_or(&tool.name);
    let binary_path = binary::find_binary(&extracted_files, temp_dir.path(), binary_name)?;

    if verbose {
        println!("Found binary: {}", binary_path.display());
    }

    // Install binary
    let dest = binary::install_binary(&binary_path, &config.settings.install_dir, binary_name)?;

    // Update version in config
    config.update_tool_version(&tool.name, release.tag_name.clone())?;
    config.save()?;

    println!("Installed {} to {}", tool.name, dest.display());
    Ok(())
}

pub async fn update_all_tools(config: &mut Config, verbose: bool, force: bool) -> Result<()> {
    let mut success = 0;
    let mut failed = 0;

    let tool_names: Vec<String> = config.tools.iter().map(|t| t.name.clone()).collect();

    for tool_name in tool_names {
        match update_tool(config, &tool_name, verbose, force).await {
            Ok(_) => success += 1,
            Err(e) => {
                eprintln!("Failed to update {}: {}", tool_name, e);
                failed += 1;
            }
        }
    }

    println!("\nSummary: {} updated, {} failed", success, failed);
    Ok(())
}

pub fn remove_tool(config: &mut Config, tool_name: &str) -> Result<()> {
    config.remove_tool(tool_name)?;
    config.save()?;
    println!("Removed tool '{}'", tool_name);
    println!(
        "Note: Binary in {} not removed",
        config.settings.install_dir.display()
    );
    Ok(())
}

pub fn list_tools(config: &Config) -> Result<()> {
    if config.tools.is_empty() {
        println!("No tools configured.");
        println!("Add a tool with: oktofetch add <github-repo>");
        return Ok(());
    }

    println!("Configured tools:\n");
    for tool in &config.tools {
        let version_str = tool
            .version
            .as_ref()
            .map(|v| format!(" ({})", v))
            .unwrap_or_default();
        println!("  {:<20} {}{}", tool.name, tool.repo, version_str);
        if let Some(binary) = &tool.binary_name {
            println!("  {:<20} binary: {}", "", binary);
        }
    }

    Ok(())
}

fn parse_repo(input: &str) -> Result<String> {
    // Handle full GitHub URLs
    if input.starts_with("http://") || input.starts_with("https://") {
        let url = input
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let parts: Vec<&str> = url.split('/').collect();

        if parts.len() >= 3 && parts[0] == "github.com" {
            return Ok(format!("{}/{}", parts[1], parts[2]));
        }
    }

    // Validate owner/repo format
    if input.split('/').count() == 2 {
        return Ok(input.to_string());
    }

    Err(OktofetchError::Other(format!(
        "Invalid repository format: {}. Expected 'owner/repo' or GitHub URL",
        input
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_priority() {
        // Test tar.gz variants (highest priority)
        assert_eq!(asset_priority("myapp.tar.gz"), 0);
        assert_eq!(asset_priority("myapp.tgz"), 0);
        assert_eq!(asset_priority("MYAPP.TAR.GZ"), 0); // Case insensitive
        assert_eq!(asset_priority("MYAPP.TGZ"), 0);

        // Test zip (second priority)
        assert_eq!(asset_priority("myapp.zip"), 1);
        assert_eq!(asset_priority("MYAPP.ZIP"), 1);

        // Test other formats (lowest priority)
        assert_eq!(asset_priority("myapp.7z"), 2);
        assert_eq!(asset_priority("myapp.rar"), 2);
        assert_eq!(asset_priority("myapp.tar"), 2);
        assert_eq!(asset_priority("myapp.exe"), 2);
    }

    #[test]
    fn test_parse_repo_simple_format() {
        assert_eq!(parse_repo("owner/repo").unwrap(), "owner/repo");
        assert_eq!(parse_repo("derailed/k9s").unwrap(), "derailed/k9s");
        assert_eq!(parse_repo("vmware/govmomi").unwrap(), "vmware/govmomi");
    }

    #[test]
    fn test_parse_repo_https_url() {
        assert_eq!(
            parse_repo("https://github.com/owner/repo").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            parse_repo("https://github.com/derailed/k9s").unwrap(),
            "derailed/k9s"
        );
        assert_eq!(
            parse_repo("https://github.com/vmware/govmomi/issues").unwrap(),
            "vmware/govmomi"
        );
    }

    #[test]
    fn test_parse_repo_http_url() {
        assert_eq!(
            parse_repo("http://github.com/owner/repo").unwrap(),
            "owner/repo"
        );
    }

    #[test]
    fn test_parse_repo_invalid_format() {
        assert!(parse_repo("invalid").is_err());
        assert!(parse_repo("owner/repo/extra/parts").is_err());
        assert!(parse_repo("").is_err());
    }

    #[test]
    fn test_parse_repo_non_github_url() {
        let result = parse_repo("https://gitlab.com/owner/repo");
        // Should fail because it's not github.com
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_repo_error_message() {
        let result = parse_repo("invalid");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Invalid repository format"));
        assert!(err_msg.contains("owner/repo"));
    }

    #[test]
    fn test_tool_name_derivation_simple() {
        // Test that we correctly parse repo and derive tool name
        assert_eq!(parse_repo("owner/repo").unwrap(), "owner/repo");

        // Test URL parsing
        assert_eq!(
            parse_repo("https://github.com/owner/myrepo").unwrap(),
            "owner/myrepo"
        );
    }

    #[test]
    fn test_remove_tool_not_found() {
        let mut config = Config::default();
        let result = remove_tool(&mut config, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_tool_logic() {
        // Test the underlying logic without saving
        let mut config = Config::default();
        let tool = crate::config::Tool {
            name: "tool1".to_string(),
            repo: "owner/repo1".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };
        config.add_tool(tool).unwrap();

        // Test the remove logic directly on config
        let result = config.remove_tool("tool1");
        assert!(result.is_ok());
        assert!(config.get_tool("tool1").is_none());
    }

    #[test]
    fn test_list_tools_empty() {
        let config = Config::default();
        let result = list_tools(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_tools_with_entries() {
        let mut config = Config::default();
        let tool = crate::config::Tool {
            name: "tool1".to_string(),
            repo: "owner/repo1".to_string(),
            binary_name: Some("bin1".to_string()),
            asset_pattern: None,
            version: Some("v1.0.0".to_string()),
        };
        config.add_tool(tool).unwrap();

        let result = list_tools(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_tools_multiple_entries() {
        let mut config = Config::default();
        for i in 1..=3 {
            let tool = crate::config::Tool {
                name: format!("tool{}", i),
                repo: format!("owner/repo{}", i),
                binary_name: None,
                asset_pattern: None,
                version: None,
            };
            config.add_tool(tool).unwrap();
        }

        let result = list_tools(&config);
        assert!(result.is_ok());
        assert_eq!(config.tools.len(), 3);
    }

    #[test]
    fn test_asset_priority_sorting() {
        // Verify that tar.gz gets lowest value (highest priority)
        assert!(asset_priority("app.tar.gz") < asset_priority("app.zip"));
        assert!(asset_priority("app.zip") < asset_priority("app.7z"));

        // Verify tgz also gets highest priority
        assert_eq!(asset_priority("app.tgz"), asset_priority("app.tar.gz"));
    }

    #[tokio::test]
    async fn test_add_tool_basic() {
        let mut config = Config::default();

        // Test adding a tool
        let result = config.add_tool(crate::config::Tool {
            name: "testtool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        });

        assert!(result.is_ok());
        assert_eq!(config.tools.len(), 1);
        assert_eq!(config.tools[0].name, "testtool");
    }

    #[test]
    fn test_parse_repo_edge_cases() {
        // Test with trailing slash
        assert_eq!(
            parse_repo("https://github.com/owner/repo/").unwrap(),
            "owner/repo"
        );

        // Test with .git suffix - this is valid (owner/repo.git is still owner/repo.git)
        assert_eq!(parse_repo("owner/repo.git").unwrap(), "owner/repo.git");

        // Test empty string
        assert!(parse_repo("").is_err());

        // Test malformed inputs
        assert!(parse_repo("onlyonepart").is_err());
        assert!(parse_repo("too/many/parts/here").is_err());

        // Test with special characters that should work
        assert_eq!(parse_repo("my-org/my-repo").unwrap(), "my-org/my-repo");
        assert_eq!(
            parse_repo("org_name/repo_name").unwrap(),
            "org_name/repo_name"
        );
    }

    #[test]
    fn test_list_tools_formatting() {
        let mut config = Config::default();

        // Add tools with various configurations
        config
            .add_tool(crate::config::Tool {
                name: "tool_with_version".to_string(),
                repo: "owner/repo1".to_string(),
                binary_name: Some("custom_bin".to_string()),
                asset_pattern: None,
                version: Some("v1.0.0".to_string()),
            })
            .unwrap();

        config
            .add_tool(crate::config::Tool {
                name: "tool_without_version".to_string(),
                repo: "owner/repo2".to_string(),
                binary_name: None,
                asset_pattern: None,
                version: None,
            })
            .unwrap();

        let result = list_tools(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_tool_updates_config() {
        let mut config = Config::default();

        // Add multiple tools
        for i in 1..=3 {
            config
                .add_tool(crate::config::Tool {
                    name: format!("tool{}", i),
                    repo: format!("owner/repo{}", i),
                    binary_name: None,
                    asset_pattern: None,
                    version: None,
                })
                .unwrap();
        }

        assert_eq!(config.tools.len(), 3);

        // Remove middle tool
        config.remove_tool("tool2").unwrap();
        assert_eq!(config.tools.len(), 2);
        assert!(config.get_tool("tool1").is_some());
        assert!(config.get_tool("tool2").is_none());
        assert!(config.get_tool("tool3").is_some());
    }

    #[test]
    fn test_parse_repo_url_variations() {
        // Test various URL formats
        let test_cases = vec![
            ("https://github.com/owner/repo", "owner/repo"),
            ("http://github.com/owner/repo", "owner/repo"),
            ("https://github.com/owner/repo/releases", "owner/repo"),
            ("https://github.com/owner/repo/tree/main", "owner/repo"),
        ];

        for (input, expected) in test_cases {
            assert_eq!(
                parse_repo(input).unwrap(),
                expected,
                "Failed for input: {}",
                input
            );
        }
    }
}
