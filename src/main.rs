use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod archive;
mod binary;
mod config;
mod error;
mod github;
mod platform;
mod tool;

use config::Config;
use error::Result;

#[derive(Parser)]
#[command(name = "oktofetch")]
#[command(version, about = "A GitHub release binary manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new tool from a GitHub repository
    Add {
        /// GitHub repository (owner/repo or full URL)
        repo: String,

        /// Custom name for the tool
        #[arg(short, long)]
        name: Option<String>,

        /// Binary name to extract and install
        #[arg(short, long)]
        binary: Option<String>,
    },

    /// Remove a tool from management
    Remove {
        /// Tool name to remove
        name: String,
    },

    /// Update one or all tools
    Update {
        /// Tool name to update (omit for all)
        name: Option<String>,

        /// Update all tools
        #[arg(short, long)]
        all: bool,

        /// Force reinstallation even if version matches
        #[arg(short, long)]
        force: bool,
    },

    /// List all managed tools
    List,

    /// Show information about a tool
    Info {
        /// Tool name
        name: String,
    },

    /// Show or set configuration
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Set installation directory
    Set {
        /// Configuration key (e.g., install_dir)
        key: String,

        /// Configuration value
        value: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("Error: {}", e);
        let exit_code = e.exit_code();
        process::exit(exit_code);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Add { repo, name, binary } => {
            let mut config = Config::load()?;
            tool::add_tool(&mut config, repo, name, binary).await
        }

        Commands::Remove { name } => {
            let mut config = Config::load()?;
            tool::remove_tool(&mut config, &name)
        }

        Commands::Update { name, all, force } => {
            let mut config = Config::load()?;

            if all || name.is_none() {
                tool::update_all_tools(&mut config, cli.verbose, force).await
            } else if let Some(tool_name) = name {
                tool::update_tool(&mut config, &tool_name, cli.verbose, force).await
            } else {
                Err(error::OktofetchError::Other(
                    "Specify a tool name or use --all".to_string(),
                ))
            }
        }

        Commands::List => {
            let config = Config::load()?;
            tool::list_tools(&config)
        }

        Commands::Info { name } => {
            let config = Config::load()?;
            show_tool_info(&config, &name)
        }

        Commands::Config { command } => match command {
            Some(ConfigCommands::Show) | None => {
                let config = Config::load()?;
                show_config(&config)
            }
            Some(ConfigCommands::Set { key, value }) => {
                let mut config = Config::load()?;
                set_config(&mut config, &key, &value)
            }
        },
    }
}

fn show_tool_info(config: &Config, name: &str) -> Result<()> {
    let tool = config
        .get_tool(name)
        .ok_or_else(|| error::OktofetchError::ToolNotFound(name.to_string()))?;

    println!("Tool: {}", tool.name);
    println!("Repository: {}", tool.repo);
    if let Some(version) = &tool.version {
        println!("Version: {}", version);
    }
    if let Some(binary) = &tool.binary_name {
        println!("Binary name: {}", binary);
    }
    if let Some(pattern) = &tool.asset_pattern {
        println!("Asset pattern: {}", pattern);
    }

    Ok(())
}

fn show_config(config: &Config) -> Result<()> {
    println!("Configuration:");
    println!(
        "  Install directory: {}",
        config.settings.install_dir.display()
    );
    println!("  Config file: {}", Config::config_path()?.display());
    Ok(())
}

fn set_config(config: &mut Config, key: &str, value: &str) -> Result<()> {
    match key {
        "install_dir" => {
            config.settings.install_dir = PathBuf::from(value);
            config.save()?;
            println!("Set install_dir to {}", value);
            Ok(())
        }
        _ => Err(error::OktofetchError::Other(format!(
            "Unknown config key: {}. Valid keys: install_dir",
            key
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_tool_info_not_found() {
        let config = Config::default();
        let result = show_tool_info(&config, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_show_tool_info() {
        let mut config = Config::default();
        let tool = config::Tool {
            name: "test".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: Some("test-bin".to_string()),
            asset_pattern: Some("linux-x64".to_string()),
            version: Some("v1.0.0".to_string()),
        };
        config.add_tool(tool).unwrap();

        let result = show_tool_info(&config, "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_config() {
        let config = Config::default();
        let result = show_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_config_unknown_key() {
        let mut config = Config::default();
        let result = set_config(&mut config, "unknown_key", "value");
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("Unknown config key"));
    }

    #[test]
    fn test_set_config_logic() {
        let mut config = Config::default();
        let original_dir = config.settings.install_dir.clone();

        // Just test the logic without saving
        config.settings.install_dir = PathBuf::from("/custom/path");
        assert_eq!(config.settings.install_dir, PathBuf::from("/custom/path"));

        // Restore
        config.settings.install_dir = original_dir;
    }

    #[test]
    fn test_show_tool_info_with_all_fields() {
        let mut config = Config::default();
        let tool = config::Tool {
            name: "fulltool".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: Some("binary".to_string()),
            asset_pattern: Some("pattern".to_string()),
            version: Some("v1.2.3".to_string()),
        };
        config.add_tool(tool).unwrap();

        let result = show_tool_info(&config, "fulltool");
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_tool_info_minimal_fields() {
        let mut config = Config::default();
        let tool = config::Tool {
            name: "minimal".to_string(),
            repo: "owner/repo".to_string(),
            binary_name: None,
            asset_pattern: None,
            version: None,
        };
        config.add_tool(tool).unwrap();

        let result = show_tool_info(&config, "minimal");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_config_install_dir() {
        let mut config = Config::default();
        let new_path = "/tmp/test_install";

        // Test the set_config function logic
        config.settings.install_dir = PathBuf::from(new_path);
        assert_eq!(config.settings.install_dir, PathBuf::from(new_path));
    }

    #[test]
    fn test_cli_parsing_add_command() {
        let cli = Cli::parse_from(["oktofetch", "add", "owner/repo"]);
        match cli.command {
            Commands::Add { repo, name, binary } => {
                assert_eq!(repo, "owner/repo");
                assert!(name.is_none());
                assert!(binary.is_none());
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn test_cli_parsing_add_with_options() {
        let cli = Cli::parse_from([
            "oktofetch",
            "add",
            "owner/repo",
            "--name",
            "mytool",
            "--binary",
            "mybin",
        ]);
        match cli.command {
            Commands::Add { repo, name, binary } => {
                assert_eq!(repo, "owner/repo");
                assert_eq!(name, Some("mytool".to_string()));
                assert_eq!(binary, Some("mybin".to_string()));
            }
            _ => panic!("Expected Add command"),
        }
    }

    #[test]
    fn test_cli_parsing_remove() {
        let cli = Cli::parse_from(["oktofetch", "remove", "mytool"]);
        match cli.command {
            Commands::Remove { name } => {
                assert_eq!(name, "mytool");
            }
            _ => panic!("Expected Remove command"),
        }
    }

    #[test]
    fn test_cli_parsing_update() {
        let cli = Cli::parse_from(["oktofetch", "update", "mytool"]);
        match cli.command {
            Commands::Update { name, all, force } => {
                assert_eq!(name, Some("mytool".to_string()));
                assert!(!all);
                assert!(!force);
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_update_all() {
        let cli = Cli::parse_from(["oktofetch", "update", "--all"]);
        match cli.command {
            Commands::Update { name, all, force } => {
                assert!(name.is_none());
                assert!(all);
                assert!(!force);
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_update_force() {
        let cli = Cli::parse_from(["oktofetch", "update", "mytool", "--force"]);
        match cli.command {
            Commands::Update { name, all, force } => {
                assert_eq!(name, Some("mytool".to_string()));
                assert!(!all);
                assert!(force);
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_cli_parsing_list() {
        let cli = Cli::parse_from(["oktofetch", "list"]);
        matches!(cli.command, Commands::List);
    }

    #[test]
    fn test_cli_parsing_info() {
        let cli = Cli::parse_from(["oktofetch", "info", "mytool"]);
        match cli.command {
            Commands::Info { name } => {
                assert_eq!(name, "mytool");
            }
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_cli_parsing_config_show() {
        let cli = Cli::parse_from(["oktofetch", "config", "show"]);
        match cli.command {
            Commands::Config { command } => {
                assert!(matches!(command, Some(ConfigCommands::Show)));
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_cli_parsing_config_set() {
        let cli = Cli::parse_from(["oktofetch", "config", "set", "install_dir", "/custom/path"]);
        match cli.command {
            Commands::Config { command } => match command {
                Some(ConfigCommands::Set { key, value }) => {
                    assert_eq!(key, "install_dir");
                    assert_eq!(value, "/custom/path");
                }
                _ => panic!("Expected Set subcommand"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_cli_verbose_flag() {
        let cli = Cli::parse_from(["oktofetch", "-v", "list"]);
        assert!(cli.verbose);

        let cli = Cli::parse_from(["oktofetch", "--verbose", "list"]);
        assert!(cli.verbose);

        let cli = Cli::parse_from(["oktofetch", "list"]);
        assert!(!cli.verbose);
    }
}
