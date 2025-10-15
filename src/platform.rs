use crate::error::Result;

pub fn validate_platform() -> Result<()> {
    if std::env::consts::OS != "linux" {
        return Err(crate::error::OktofetchError::Other(format!(
            "Unsupported OS: {}",
            std::env::consts::OS
        )));
    }
    if std::env::consts::ARCH != "x86_64" {
        return Err(crate::error::OktofetchError::Other(format!(
            "Unsupported arch: {}",
            std::env::consts::ARCH
        )));
    }
    Ok(())
}

/// Checks if an asset name matches Linux x86_64 platform requirements.
/// Looks for "linux" and one of: "x86_64", "amd64", or "x64".
pub fn matches_asset_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();

    name_lower.contains("linux")
        && (name_lower.contains("x86_64")
            || name_lower.contains("amd64")
            || name_lower.contains("x64"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_platform() {
        // This test will pass on Linux x86_64, fail elsewhere
        // That's expected - the tool only supports Linux x86_64
        let result = validate_platform();
        if std::env::consts::OS == "linux" && std::env::consts::ARCH == "x86_64" {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_asset_matching_positive() {
        // Should match these
        assert!(matches_asset_name("myapp-linux-x86_64.tar.gz"));
        assert!(matches_asset_name("myapp-linux-amd64.tar.gz"));
        assert!(matches_asset_name("tool_Linux_x64.zip"));
        assert!(matches_asset_name("MYAPP-LINUX-X86_64.TAR.GZ")); // Case insensitive
    }

    #[test]
    fn test_asset_matching_negative() {
        // Should NOT match these - wrong OS
        assert!(!matches_asset_name("myapp-darwin-x86_64.tar.gz"));
        assert!(!matches_asset_name("myapp-windows-x86_64.zip"));
        assert!(!matches_asset_name("myapp-macos-x86_64.tar.gz"));

        // Should NOT match these - wrong architecture
        assert!(!matches_asset_name("myapp-linux-arm64.tar.gz"));
        assert!(!matches_asset_name("myapp-linux-aarch64.tar.gz"));
        assert!(!matches_asset_name("myapp-linux-arm.tar.gz"));

        // Should NOT match these - missing required parts
        assert!(!matches_asset_name("myapp-x86_64.tar.gz")); // No "linux"
        assert!(!matches_asset_name("myapp-linux.tar.gz")); // No arch
    }

    #[test]
    fn test_asset_matching_edge_cases() {
        // Edge cases with different formats
        assert!(matches_asset_name("linux_x86_64.tar.gz")); // underscore
        assert!(matches_asset_name("linux.x86_64")); // dot separator
        assert!(matches_asset_name("X86_64-LINUX")); // different order, case insensitive

        // These contain linux and x86_64 so they match (substring matching)
        assert!(matches_asset_name("notlinux-x86_64")); // contains "linux" and "x86_64"
        assert!(matches_asset_name("linux-notx86_64")); // contains both "linux" and "x86_64"

        // Should not match - missing correct architecture
        assert!(!matches_asset_name("linux-i386")); // wrong arch
        assert!(!matches_asset_name("linux-arm")); // wrong arch
        assert!(!matches_asset_name("linux")); // no arch at all
    }

    #[test]
    fn test_validate_platform_error_messages() {
        let result = validate_platform();

        if std::env::consts::OS != "linux" {
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(format!("{}", err).contains("Unsupported OS"));
        } else if std::env::consts::ARCH != "x86_64" {
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(format!("{}", err).contains("Unsupported arch"));
        }
    }

    #[test]
    fn test_matches_asset_name_case_variations() {
        // Test various case combinations
        assert!(matches_asset_name("LINUX-X86_64.tar.gz"));
        assert!(matches_asset_name("Linux-x86_64.tar.gz"));
        assert!(matches_asset_name("linux-X86_64.tar.gz"));
        assert!(matches_asset_name("LiNuX-x86_64.tar.gz"));

        // AMD64 variants
        assert!(matches_asset_name("linux-AMD64.tar.gz"));
        assert!(matches_asset_name("LINUX-amd64.tar.gz"));
        assert!(matches_asset_name("Linux-AmD64.zip"));
    }

    #[test]
    fn test_matches_asset_name_x64_variants() {
        // Test x64 (without underscore)
        assert!(matches_asset_name("myapp-linux-x64.tar.gz"));
        assert!(matches_asset_name("tool-Linux-X64.zip"));
        assert!(matches_asset_name("app_linux_x64.tgz"));
    }

    #[test]
    fn test_matches_asset_name_complex_names() {
        // Real-world complex names
        assert!(matches_asset_name("myapp-v1.0.0-linux-x86_64.tar.gz"));
        assert!(matches_asset_name("tool_1.2.3_Linux_amd64.zip"));
        assert!(matches_asset_name("app-nightly-2024-linux-x64.tgz"));
        assert!(matches_asset_name("binary-linux-musl-x86_64.tar.gz"));
    }

    #[test]
    fn test_matches_asset_name_false_positives() {
        // Should NOT match - incomplete or wrong patterns
        assert!(!matches_asset_name("myapp.tar.gz")); // no OS or arch
        assert!(!matches_asset_name("x86_64.tar.gz")); // no OS
        assert!(!matches_asset_name("linux.tar.gz")); // no arch
        assert!(!matches_asset_name("windows-x86_64.exe")); // wrong OS
        assert!(!matches_asset_name("macos-x86_64.dmg")); // wrong OS
        assert!(!matches_asset_name("linux-arm64.tar.gz")); // wrong arch
        assert!(!matches_asset_name("darwin-amd64.tar.gz")); // wrong OS
    }

    #[test]
    fn test_matches_asset_name_substring_behavior() {
        // These should match because contains() finds substrings
        assert!(matches_asset_name("prefix-linux-x86_64-suffix.tar.gz"));
        assert!(matches_asset_name("linux_x86_64"));
        assert!(matches_asset_name("aaa-linux-bbb-x86_64-ccc"));
    }
}
