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
    } else if file_name.ends_with(".zip") {
        extract_zip(archive_path, dest_dir)
    } else {
        Err(OktofetchError::ExtractionFailed(format!(
            "Unsupported archive format: {}",
            file_name
        )))
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
        entry.unpack(&dest_path)?;

        if let Some(path_str) = path.to_str() {
            extracted_files.push(path_str.to_string());
        }
    }

    Ok(extracted_files)
}

fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<Vec<String>> {
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
        }

        extracted_files.push(file.name().to_string());
    }

    Ok(extracted_files)
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
}
