use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, OktofetchError>;

#[derive(Error, Debug)]
pub enum OktofetchError {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("GitHub API error: {0}")]
    GithubApi(String),

    #[error("Repository not found: {0}")]
    RepoNotFound(String),

    #[error("No suitable release for {platform} {arch}")]
    NoSuitableRelease { platform: String, arch: String },

    #[error("Config error: {0} at {1}")]
    ConfigError(String, PathBuf),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Binary not found: {0}")]
    BinaryNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("{0}")]
    Other(String),
}

impl OktofetchError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ToolNotFound(_) => 1,
            Self::GithubApi(_) => 2,
            Self::RepoNotFound(_) => 1,
            Self::NoSuitableRelease { .. } => 3,
            Self::ConfigError(_, _) => 4,
            Self::DownloadFailed(_) => 7,
            Self::ExtractionFailed(_) => 8,
            Self::BinaryNotFound(_) => 9,
            Self::Io(_) => 10,
            Self::Reqwest(_) => 11,
            Self::Other(_) => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_exit_codes() {
        assert_eq!(
            OktofetchError::ToolNotFound("test".to_string()).exit_code(),
            1
        );
        assert_eq!(
            OktofetchError::GithubApi("error".to_string()).exit_code(),
            2
        );
        assert_eq!(
            OktofetchError::NoSuitableRelease {
                platform: "Linux".to_string(),
                arch: "x86_64".to_string()
            }
            .exit_code(),
            3
        );
        assert_eq!(
            OktofetchError::ConfigError("error".to_string(), PathBuf::from("/tmp")).exit_code(),
            4
        );
        assert_eq!(
            OktofetchError::DownloadFailed("error".to_string()).exit_code(),
            7
        );
        assert_eq!(
            OktofetchError::ExtractionFailed("error".to_string()).exit_code(),
            8
        );
        assert_eq!(
            OktofetchError::BinaryNotFound("error".to_string()).exit_code(),
            9
        );
    }

    #[test]
    fn test_error_messages() {
        let err = OktofetchError::ToolNotFound("myapp".to_string());
        assert!(format!("{}", err).contains("myapp"));

        let err = OktofetchError::RepoNotFound("owner/repo".to_string());
        assert!(format!("{}", err).contains("owner/repo"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let okto_err: OktofetchError = io_err.into();
        assert_eq!(okto_err.exit_code(), 10);
    }

    #[test]
    fn test_all_error_variants_display() {
        let errors = vec![
            OktofetchError::ToolNotFound("mytool".to_string()),
            OktofetchError::GithubApi("API error".to_string()),
            OktofetchError::RepoNotFound("owner/repo".to_string()),
            OktofetchError::NoSuitableRelease {
                platform: "Linux".to_string(),
                arch: "x86_64".to_string(),
            },
            OktofetchError::ConfigError(
                "config error".to_string(),
                std::path::PathBuf::from("/path"),
            ),
            OktofetchError::DownloadFailed("download error".to_string()),
            OktofetchError::ExtractionFailed("extract error".to_string()),
            OktofetchError::BinaryNotFound("binary not found".to_string()),
            OktofetchError::Other("other error".to_string()),
        ];

        for error in errors {
            let display = format!("{}", error);
            assert!(!display.is_empty());
            assert!(error.exit_code() > 0);
        }
    }

    #[test]
    fn test_error_exit_code_uniqueness() {
        // Verify different error types have different exit codes (where appropriate)
        let tool_not_found = OktofetchError::ToolNotFound("test".to_string()).exit_code();
        let github_api = OktofetchError::GithubApi("test".to_string()).exit_code();
        let no_release = OktofetchError::NoSuitableRelease {
            platform: "Linux".to_string(),
            arch: "x86_64".to_string(),
        }
        .exit_code();

        assert_ne!(tool_not_found, github_api);
        assert_ne!(github_api, no_release);
    }

    #[test]
    fn test_no_suitable_release_display() {
        let err = OktofetchError::NoSuitableRelease {
            platform: "Linux".to_string(),
            arch: "x86_64".to_string(),
        };

        let display = format!("{}", err);
        assert!(display.contains("Linux"));
        assert!(display.contains("x86_64"));
    }

    #[test]
    fn test_config_error_display() {
        let err = OktofetchError::ConfigError(
            "parse error".to_string(),
            std::path::PathBuf::from("/tmp/config.toml"),
        );

        let display = format!("{}", err);
        assert!(display.contains("parse error"));
        assert!(display.contains("/tmp/config.toml"));
    }
}
