use crate::error::{OktofetchError, Result};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Check if a file is an ELF binary by examining its magic header
fn is_elf_binary(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut header = [0u8; 4];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    // ELF magic number is 0x7F 'E' 'L' 'F'
    header == [0x7F, b'E', b'L', b'F']
}

pub fn find_binary(
    extracted_files: &[String],
    extract_dir: &Path,
    tool_name: &str,
) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    // Look for executable files (either by permission or by ELF signature)
    let mut executables = Vec::new();

    for file_name in extracted_files {
        let file_path = extract_dir.join(file_name);

        if !file_path.is_file() {
            continue;
        }

        // Check if file has execute permission OR is an ELF binary
        // Some archives don't preserve execute bits, so we check the ELF header too
        let is_executable = if let Ok(metadata) = fs::metadata(&file_path) {
            let permissions = metadata.permissions();
            permissions.mode() & 0o111 != 0
        } else {
            false
        };

        if is_executable || is_elf_binary(&file_path) {
            executables.push(file_path);
        }
    }

    if executables.is_empty() {
        return Err(OktofetchError::BinaryNotFound(
            "No executable files found in archive".to_string(),
        ));
    }

    // Try to find binary matching tool name
    for exe in &executables {
        if let Some(file_name) = exe.file_name().and_then(|n| n.to_str())
            && file_name.contains(tool_name)
        {
            return Ok(exe.clone());
        }
    }

    // If only one executable, use it
    if executables.len() == 1 {
        return Ok(executables[0].clone());
    }

    // Multiple executables found, can't decide
    Err(OktofetchError::BinaryNotFound(format!(
        "Multiple executables found, please specify binary_name in config. Found: {:?}",
        executables
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .collect::<Vec<_>>()
    )))
}

pub fn install_binary(binary_path: &Path, install_dir: &Path, name: &str) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    if !install_dir.exists() {
        fs::create_dir_all(install_dir)?;
    }

    let dest = install_dir.join(name);
    fs::copy(binary_path, &dest)?;

    // Make executable
    let mut perms = fs::metadata(&dest)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms)?;

    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_find_binary_no_executables() {
        let temp_dir = TempDir::new().unwrap();
        let files = vec!["readme.txt".to_string(), "config.json".to_string()];

        for file in &files {
            File::create(temp_dir.path().join(file)).unwrap();
        }

        let result = find_binary(&files, temp_dir.path(), "myapp");
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("No executable files found"));
    }

    #[test]
    fn test_find_binary_single_executable() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let exe_path = temp_dir.path().join("myapp");
        File::create(&exe_path).unwrap();

        let mut perms = fs::metadata(&exe_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe_path, perms).unwrap();

        let files = vec!["myapp".to_string()];
        let result = find_binary(&files, temp_dir.path(), "myapp");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().file_name().unwrap(), "myapp");
    }

    #[test]
    fn test_find_binary_matching_name() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();

        for name in &["readme", "myapp", "helper"] {
            let exe_path = temp_dir.path().join(name);
            File::create(&exe_path).unwrap();
            let mut perms = fs::metadata(&exe_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe_path, perms).unwrap();
        }

        let files = vec![
            "readme".to_string(),
            "myapp".to_string(),
            "helper".to_string(),
        ];
        let result = find_binary(&files, temp_dir.path(), "myapp");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().file_name().unwrap(), "myapp");
    }

    #[test]
    fn test_install_binary() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("bin");
        let source_path = temp_dir.path().join("source");

        fs::write(&source_path, b"binary content").unwrap();

        let result = install_binary(&source_path, &install_dir, "myapp");
        assert!(result.is_ok());

        let dest = result.unwrap();
        assert!(dest.exists());
        assert_eq!(dest.file_name().unwrap(), "myapp");
        assert_eq!(fs::read_to_string(&dest).unwrap(), "binary content");

        let perms = fs::metadata(&dest).unwrap().permissions();
        assert_ne!(perms.mode() & 0o111, 0);
    }

    #[test]
    fn test_install_binary_creates_dir() {
        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("does/not/exist");
        let source_path = temp_dir.path().join("source");

        fs::write(&source_path, b"content").unwrap();

        // Should create the directory
        let result = install_binary(&source_path, &install_dir, "myapp");
        assert!(result.is_ok());
        assert!(install_dir.exists());
    }

    #[test]
    fn test_find_binary_multiple_executables_no_match() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();

        // Create multiple executables with different names
        for name in &["exe1", "exe2", "exe3"] {
            let exe_path = temp_dir.path().join(name);
            File::create(&exe_path).unwrap();
            let mut perms = fs::metadata(&exe_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe_path, perms).unwrap();
        }

        let files = vec!["exe1".to_string(), "exe2".to_string(), "exe3".to_string()];

        // Look for a tool name that doesn't match any executable
        let result = find_binary(&files, temp_dir.path(), "nonexistent");
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("Multiple executables found"));
    }

    #[test]
    fn test_find_binary_with_non_executable_files() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();

        // Create some non-executable files
        for name in &["readme.txt", "config.json"] {
            File::create(temp_dir.path().join(name)).unwrap();
        }

        // Create one executable
        let exe_path = temp_dir.path().join("myapp");
        File::create(&exe_path).unwrap();
        let mut perms = fs::metadata(&exe_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe_path, perms).unwrap();

        let files = vec![
            "readme.txt".to_string(),
            "myapp".to_string(),
            "config.json".to_string(),
        ];

        // Should find the only executable
        let result = find_binary(&files, temp_dir.path(), "myapp");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().file_name().unwrap(), "myapp");
    }

    #[test]
    fn test_install_binary_overwrites_existing() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("bin");
        fs::create_dir(&install_dir).unwrap();

        let source_path = temp_dir.path().join("source");
        fs::write(&source_path, b"new content").unwrap();

        // Create existing file
        let dest = install_dir.join("myapp");
        fs::write(&dest, b"old content").unwrap();

        // Install should overwrite
        let result = install_binary(&source_path, &install_dir, "myapp");
        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new content");

        // Check permissions
        let perms = fs::metadata(&dest).unwrap().permissions();
        assert_ne!(perms.mode() & 0o111, 0);
    }

    #[test]
    fn test_find_binary_elf_without_execute_permission() {
        // Test finding an ELF binary that doesn't have execute permissions set
        // This is common with some archive creators (like terragrunt releases)
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("myapp");

        // Create a fake ELF binary (with ELF magic header) but NO execute permission
        let mut elf_data = vec![0x7F, b'E', b'L', b'F'];
        elf_data.extend_from_slice(&[0u8; 100]); // Add some padding
        fs::write(&binary_path, &elf_data).unwrap();

        // Explicitly ensure no execute permission (should be default, but be explicit)
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path).unwrap().permissions();
        perms.set_mode(0o644); // rw-r--r--, no execute bits
        fs::set_permissions(&binary_path, perms).unwrap();

        let files = vec!["myapp".to_string()];
        let result = find_binary(&files, temp_dir.path(), "myapp");

        // Should still find the binary due to ELF header detection
        assert!(
            result.is_ok(),
            "Should find ELF binary without execute permission"
        );
        assert_eq!(result.unwrap().file_name().unwrap(), "myapp");
    }

    #[test]
    fn test_is_elf_binary() {
        let temp_dir = TempDir::new().unwrap();

        // Test with ELF binary
        let elf_path = temp_dir.path().join("elf_binary");
        let mut elf_data = vec![0x7F, b'E', b'L', b'F'];
        elf_data.extend_from_slice(&[0u8; 100]);
        fs::write(&elf_path, &elf_data).unwrap();
        assert!(is_elf_binary(&elf_path));

        // Test with non-ELF file
        let text_path = temp_dir.path().join("text_file");
        fs::write(&text_path, b"This is just a text file").unwrap();
        assert!(!is_elf_binary(&text_path));

        // Test with empty file
        let empty_path = temp_dir.path().join("empty");
        File::create(&empty_path).unwrap();
        assert!(!is_elf_binary(&empty_path));

        // Test with nonexistent file
        let nonexistent = temp_dir.path().join("nonexistent");
        assert!(!is_elf_binary(&nonexistent));
    }
}
