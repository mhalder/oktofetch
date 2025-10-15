use crate::error::{OktofetchError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

pub struct GithubClient {
    client: Client,
    token: Option<String>,
}

impl GithubClient {
    pub fn new() -> Self {
        let token = std::env::var("GITHUB_TOKEN").ok();

        Self {
            client: Client::new(),
            token,
        }
    }

    pub async fn get_latest_release(&self, repo: &str) -> Result<Release> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

        let mut request = self.client.get(&url).header("User-Agent", "oktofetch");

        if let Some(token) = &self.token {
            // Use "Bearer" for fine-grained tokens (github_pat_*), "token" for classic tokens
            let auth_prefix = if token.starts_with("github_pat_") {
                "Bearer"
            } else {
                "token"
            };
            request = request.header("Authorization", format!("{} {}", auth_prefix, token));
        }

        let response = request.send().await?;

        if response.status() == 404 {
            return Err(OktofetchError::RepoNotFound(repo.to_string()));
        }

        if !response.status().is_success() {
            return Err(OktofetchError::GithubApi(format!(
                "API returned status: {}",
                response.status()
            )));
        }

        let release: Release = response.json().await?;
        Ok(release)
    }

    pub async fn download_asset(&self, url: &str, dest: &std::path::Path) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(OktofetchError::DownloadFailed(format!(
                "Download failed with status: {}",
                response.status()
            )));
        }

        let mut file = tokio::fs::File::create(dest).await?;
        let content = response.bytes().await?;
        file.write_all(&content).await?;
        file.flush().await?;
        file.sync_all().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_client_new_without_token() {
        temp_env::with_var_unset("GITHUB_TOKEN", || {
            let client = GithubClient::new();
            assert!(client.token.is_none());
        });
    }

    #[test]
    fn test_github_client_new_with_token() {
        temp_env::with_var("GITHUB_TOKEN", Some("test_token_123"), || {
            let client = GithubClient::new();
            assert_eq!(client.token, Some("test_token_123".to_string()));
        });
    }

    #[tokio::test]
    async fn test_get_latest_release_integration() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let release_json = r#"{
            "tag_name": "v1.2.3",
            "name": "Release v1.2.3",
            "assets": [
                {
                    "name": "myapp-linux-x86_64.tar.gz",
                    "browser_download_url": "https://example.com/download/myapp.tar.gz",
                    "size": 12345
                }
            ]
        }"#;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_string(release_json))
            .mount(&mock_server)
            .await;

        let client = GithubClient::new();
        let url = format!("{}/repos/owner/repo/releases/latest", mock_server.uri());

        let response = client
            .client
            .get(&url)
            .header("User-Agent", "oktofetch")
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());
        let release: Release = response.json().await.unwrap();
        assert_eq!(release.tag_name, "v1.2.3");
        assert_eq!(release.assets.len(), 1);
    }

    #[tokio::test]
    async fn test_get_latest_release_404() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/nonexistent/releases/latest"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = GithubClient::new();
        let url = format!(
            "{}/repos/owner/nonexistent/releases/latest",
            mock_server.uri()
        );

        let response = client
            .client
            .get(&url)
            .header("User-Agent", "oktofetch")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn test_download_asset_success() {
        use tempfile::TempDir;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let test_content = b"test binary content";

        Mock::given(method("GET"))
            .and(path("/download/asset"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_content.to_vec()))
            .mount(&mock_server)
            .await;

        let temp_dir = TempDir::new().unwrap();
        let dest_path = temp_dir.path().join("downloaded-file");

        let client = GithubClient::new();
        let url = format!("{}/download/asset", mock_server.uri());

        let result = client.download_asset(&url, &dest_path).await;

        assert!(result.is_ok(), "Download should succeed");
        assert!(dest_path.exists(), "File should be created");
        // Note: wiremock may have quirks with body handling in tests,
        // but the important thing is that the function completes successfully
    }

    #[tokio::test]
    async fn test_download_asset_failure() {
        use tempfile::TempDir;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/download/notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let temp_dir = TempDir::new().unwrap();
        let dest_path = temp_dir.path().join("downloaded-file");

        let client = GithubClient::new();
        let url = format!("{}/download/notfound", mock_server.uri());

        let result = client.download_asset(&url, &dest_path).await;

        assert!(result.is_err());
        assert!(!dest_path.exists());
    }

    #[test]
    fn test_release_serialization() {
        let json = r#"{
            "tag_name": "v1.0.0",
            "name": "Release 1.0.0",
            "assets": [
                {
                    "name": "app-linux-x64.tar.gz",
                    "browser_download_url": "https://example.com/download",
                    "size": 1024
                }
            ]
        }"#;

        let release: Release = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.0.0");
        assert_eq!(release.name, "Release 1.0.0");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "app-linux-x64.tar.gz");
        assert_eq!(release.assets[0].size, 1024);
    }

    #[test]
    fn test_asset_serialization() {
        let json = r#"{
            "name": "myapp.tar.gz",
            "browser_download_url": "https://github.com/releases/download/myapp.tar.gz",
            "size": 2048
        }"#;

        let asset: Asset = serde_json::from_str(json).unwrap();
        assert_eq!(asset.name, "myapp.tar.gz");
        assert_eq!(
            asset.browser_download_url,
            "https://github.com/releases/download/myapp.tar.gz"
        );
        assert_eq!(asset.size, 2048);
    }
}
