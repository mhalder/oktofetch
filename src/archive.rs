use crate::error::{OktofetchError, Result};
use std::fs::File;
use std::path::Path;

pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<Vec<String>> {
    let file_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| OktofetchError::ExtractionFailed("Invalid archive name".to_string()))?;

    if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        extract_tar_gz(archive_path, dest_dir)
    } else if file_name.ends_with(".tar.bz2") || file_name.ends_with(".tbz") {
        extract_tar_bz2(archive_path, dest_dir)
    } else if file_name.ends_with(".zip") {
        extract_zip(archive_path, dest_dir)
    } else {
        // Not a recognized archive format, check if it's a standalone binary
        handle_standalone_binary(archive_path, dest_dir, file_name)
    }
}

fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<Vec<String>> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = File::open(archive_path)?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);

    let mut extracted_files = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        // Security: prevent path traversal
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }

        let dest_path = dest_dir.join(&path);

        // Create parent directories if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest_path)?;

        if let Some(path_str) = path.to_str() {
            extracted_files.push(path_str.to_string());
        }
    }

    Ok(extracted_files)
}

fn extract_tar_bz2(archive_path: &Path, dest_dir: &Path) -> Result<Vec<String>> {
    use bzip2::read::BzDecoder;
    use tar::Archive;

    let file = File::open(archive_path)?;
    let bz = BzDecoder::new(file);
    let mut archive = Archive::new(bz);

    let mut extracted_files = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        // Security: prevent path traversal
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }

        let dest_path = dest_dir.join(&path);

        // Create parent directories if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest_path)?;

        if let Some(path_str) = path.to_str() {
            extracted_files.push(path_str.to_string());
        }
    }

    Ok(extracted_files)
}

fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<Vec<String>> {
    use std::os::unix::fs::PermissionsExt;
    use zip::ZipArchive;

    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        OktofetchError::ExtractionFailed(format!("Failed to open zip archive: {}", e))
    })?;

    let mut extracted_files = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            OktofetchError::ExtractionFailed(format!("Failed to extract file: {}", e))
        })?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue, // Skip invalid paths
        };

        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;

            // Check if the file is a binary and set executable permissions
            if is_elf_binary(&outpath)? {
                let mut perms = std::fs::metadata(&outpath)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&outpath, perms)?;
            }
        }

        extracted_files.push(file.name().to_string());
    }

    Ok(extracted_files)
}

fn is_elf_binary(path: &Path) -> Result<bool> {
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut header = [0u8; 4];

    // Try to read the first 4 bytes
    match file.read_exact(&mut header) {
        Ok(_) => {
            // ELF magic number is 0x7F 'E' 'L' 'F'
            Ok(header == [0x7F, b'E', b'L', b'F'])
        }
        Err(_) => Ok(false), // File too small or error, not a binary
    }
}

fn handle_standalone_binary(
    binary_path: &Path,
    dest_dir: &Path,
    file_name: &str,
) -> Result<Vec<String>> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;

    // Check file size first
    let metadata = std::fs::metadata(binary_path)?;
    if metadata.len() == 0 {
        return Err(OktofetchError::ExtractionFailed(format!(
            "Downloaded file is empty: {}",
            file_name
        )));
    }

    // Check if it's a binary file by looking for ELF header (Linux/Unix)
    let mut file = File::open(binary_path)?;
    let mut header = [0u8; 4];
    file.read_exact(&mut header)?;

    // ELF magic number is 0x7F 'E' 'L' 'F'
    let is_elf = header == [0x7F, b'E', b'L', b'F'];

    if !is_elf {
        return Err(OktofetchError::ExtractionFailed(format!(
            "Unsupported archive format: {}",
            file_name
        )));
    }

    // Check if the binary is already in the dest directory (to avoid copying to itself)
    let dest_path = dest_dir.join(file_name);

    if binary_path != dest_path {
        // Binary is in a different location, copy it
        std::fs::copy(binary_path, &dest_path)?;
    }

    // Make it executable
    let mut perms = std::fs::metadata(&dest_path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&dest_path, perms)?;

    // Return the binary as the "extracted" file
    Ok(vec![file_name.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_extract_tar_gz() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.tar.gz");

        // Create a tar.gz archive with test files
        let tar_gz = fs::File::create(&archive_path).unwrap();
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        let content = b"test content";
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "test.txt", &content[..])
            .unwrap();
        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        let files = result.unwrap();
        assert!(!files.is_empty());
        assert!(extract_dir.join("test.txt").exists());
        assert_eq!(
            fs::read_to_string(extract_dir.join("test.txt")).unwrap(),
            "test content"
        );
    }

    #[test]
    fn test_extract_zip() {
        use zip::write::{FileOptions, ZipWriter};

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.zip");

        // Create a zip archive
        let file = fs::File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("test.txt", FileOptions::default()).unwrap();
        zip.write_all(b"zip content").unwrap();
        zip.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        assert!(extract_dir.join("test.txt").exists());
        assert_eq!(
            fs::read_to_string(extract_dir.join("test.txt")).unwrap(),
            "zip content"
        );
    }

    #[test]
    fn test_extract_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.rar");
        fs::write(&archive_path, b"fake content").unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();

        let result = extract_archive(&archive_path, &extract_dir);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("Unsupported archive format"));
    }

    #[test]
    fn test_extract_nonexistent_archive() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("nonexistent.tar.gz");
        let extract_dir = temp_dir.path().join("extracted");

        let result = extract_archive(&archive_path, &extract_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_tar_gz_multiple_files() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("multi.tar.gz");

        // Create a tar.gz archive with multiple files
        let tar_gz = fs::File::create(&archive_path).unwrap();
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);

        // Add multiple files
        for i in 1..=3 {
            let mut header = tar::Header::new_gnu();
            let content = format!("content {}", i);
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, format!("file{}.txt", i), content.as_bytes())
                .unwrap();
        }
        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        for i in 1..=3 {
            assert!(extract_dir.join(format!("file{}.txt", i)).exists());
        }
    }

    #[test]
    fn test_extract_zip_with_dirs() {
        use zip::write::{FileOptions, ZipWriter};

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test_dirs.zip");

        // Create a zip archive with directories
        let file = fs::File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);

        // Add a directory
        zip.add_directory("testdir/", FileOptions::default())
            .unwrap();

        // Add a file in that directory
        zip.start_file("testdir/file.txt", FileOptions::default())
            .unwrap();
        zip.write_all(b"content").unwrap();
        zip.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        assert!(extract_dir.join("testdir").exists());
        assert!(extract_dir.join("testdir/file.txt").exists());
    }

    #[test]
    fn test_extract_invalid_filename() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with no extension
        let archive_path = temp_dir.path().join("noextension");
        fs::write(&archive_path, b"fake content").unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();

        let result = extract_archive(&archive_path, &extract_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_corrupt_tar_gz() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("corrupt.tar.gz");

        // Write corrupt data
        fs::write(&archive_path, b"this is not a valid tar.gz file").unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();

        let result = extract_archive(&archive_path, &extract_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_corrupt_zip() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("corrupt.zip");

        // Write corrupt data
        fs::write(&archive_path, b"this is not a valid zip file").unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();

        let result = extract_archive(&archive_path, &extract_dir);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("Failed to open zip archive"));
    }

    #[test]
    fn test_extract_tgz_extension() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.tgz");

        // Create a .tgz archive
        let tar_gz = fs::File::create(&archive_path).unwrap();
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        let content = b"test content";
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "test.txt", &content[..])
            .unwrap();
        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        assert!(extract_dir.join("test.txt").exists());
    }

    #[test]
    fn test_extract_tar_bz2() {
        use bzip2::Compression;
        use bzip2::write::BzEncoder;
        use tar::Builder;

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.tar.bz2");

        // Create a tar.bz2 archive with test files
        let tar_bz2 = fs::File::create(&archive_path).unwrap();
        let enc = BzEncoder::new(tar_bz2, Compression::default());
        let mut tar = Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        let content = b"bz2 test content";
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "test.txt", &content[..])
            .unwrap();
        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        let files = result.unwrap();
        assert!(!files.is_empty());
        assert!(extract_dir.join("test.txt").exists());
        assert_eq!(
            fs::read_to_string(extract_dir.join("test.txt")).unwrap(),
            "bz2 test content"
        );
    }

    #[test]
    fn test_extract_tbz_extension() {
        use bzip2::Compression;
        use bzip2::write::BzEncoder;
        use tar::Builder;

        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("test.tbz");

        // Create a .tbz archive
        let tar_bz2 = fs::File::create(&archive_path).unwrap();
        let enc = BzEncoder::new(tar_bz2, Compression::default());
        let mut tar = Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        let content = b"tbz content";
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "test.txt", &content[..])
            .unwrap();
        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&archive_path, &extract_dir);

        assert!(result.is_ok());
        assert!(extract_dir.join("test.txt").exists());
        assert_eq!(
            fs::read_to_string(extract_dir.join("test.txt")).unwrap(),
            "tbz content"
        );
    }

    #[test]
    fn test_extract_standalone_binary() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("test-binary");

        // Create a fake ELF binary (with ELF magic header)
        let mut elf_data = vec![0x7F, b'E', b'L', b'F'];
        elf_data.extend_from_slice(&[0u8; 100]); // Add some padding
        fs::write(&binary_path, &elf_data).unwrap();

        // Extract
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_archive(&binary_path, &extract_dir);

        assert!(result.is_ok());
        let extracted_files = result.unwrap();
        assert_eq!(extracted_files.len(), 1);
        assert_eq!(extracted_files[0], "test-binary");

        let extracted_binary = extract_dir.join("test-binary");
        assert!(extracted_binary.exists());

        // Check that it's executable
        let metadata = fs::metadata(&extracted_binary).unwrap();
        let permissions = metadata.permissions();
        assert_ne!(permissions.mode() & 0o111, 0);
    }

    #[test]
    fn test_extract_non_binary_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rar");

        // Create a file that's not an ELF binary
        fs::write(&file_path, b"not an elf binary").unwrap();

        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir(&extract_dir).unwrap();

        let result = extract_archive(&file_path, &extract_dir);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("Unsupported archive format"));
    }
}
