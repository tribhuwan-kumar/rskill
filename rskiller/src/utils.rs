use std::fs;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

/// calculate the total size of a directory
pub fn calculate_dir_size(dir: &Path) -> Result<u64> {
    let mut total_size = 0u64;
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total_size += entry.metadata()?.len();
        }
    }
    
    Ok(total_size)
}

/// format bytes as human readable size
pub fn format_size(bytes: u64, use_gb: bool) -> String {
    if use_gb {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// safely remove a directory and its contents
pub fn remove_directory(path: &Path, dry_run: bool) -> Result<()> {
    if dry_run {
        println!(" [DRY RUN] Would delete: {}", path.display());
        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(100));
        return Ok(());
    }
    
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    
    Ok(())
}

/// check if a path is a git repository
pub fn _is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// get relative path from current working directory
pub fn get_relative_path(path: &Path) -> String {
    if let Ok(current_dir) = std::env::current_dir() {
        if let Ok(relative) = path.strip_prefix(&current_dir) {
            return relative.display().to_string();
        }
    }
    path.display().to_string()
}

/// check if the current directory contains important system files
pub fn _is_system_directory(path: &Path) -> bool {
    let important_files = [
        "System",
        "Windows",
        "Program Files",
        "Applications",
        "/usr",
        "/bin",
        "/sbin",
        "/etc",
        "/var",
        "/opt",
    ];
    
    let path_str = path.to_string_lossy();
    important_files.iter().any(|&important| path_str.contains(important))
}

/// truncate a string to a maximum length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1024 * 1024, false), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024, true), "1.00 GB");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
    }

    #[test]
    fn test_calculate_dir_size() -> Result<()> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, world!")?;
        
        let size = calculate_dir_size(temp_dir.path())?;
        assert!(size > 0);
        
        Ok(())
    }
}
