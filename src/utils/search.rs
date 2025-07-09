use anyhow::{anyhow, Result};
use if_chain::if_chain;
use std::fs;

const MAX_DEPTH: usize = 10;

#[derive(Clone, Debug)]
pub struct Folder {
    pub path: String,
    pub size: Option<usize>,
    pub deleting: bool,
}

pub fn find_target_folders(start_path: &str, target_folder: &str, ignored_folders: &[&str],) -> Vec<Folder> {
    fn traverse(path: &str, target_folder: &str, folders: &mut Vec<Folder>, ignored_folders: &[&str], count: usize) {
        let normalized_path = path.replace('\\', "/");
        let folder_name = normalized_path.split('/').last().unwrap();

        if ignored_folders.contains(&folder_name) {
            return;
        }

        if folder_name == target_folder {
            folders.push(Folder {
                path: path.to_string(),
                size: calculate_folder_size(path).ok(),
                deleting: false,
            });
            return;
        }

        let metadata = fs::metadata(path);

        if_chain! {
            if count < MAX_DEPTH;
            if let Ok(metadata) = metadata;
            if metadata.is_dir();
            if let Ok(read_dir) = fs::read_dir(path);

            then {
                for dir in read_dir {
                    let child = dir.unwrap().path();
                    let child = child.to_str().unwrap();
                    traverse(child, target_folder, folders, ignored_folders, count + 1);
                }
            }
        }
    }

    let mut folders = vec![];

    traverse(start_path, target_folder, &mut folders, ignored_folders, 0);

    return folders;
}

fn calculate_folder_size(path: &str) -> Result<usize> {
    let mut total: usize = 0;

    for dir in fs::read_dir(path)? {
        let child = dir?;
        let metadata = child.metadata()?;

        if metadata.is_file() {
            total += metadata.len() as usize;
            continue;
        }

        if metadata.is_symlink() || !metadata.is_dir() {
            continue;
        }

        total += calculate_folder_size(child.path().to_str().ok_or(anyhow!("invalid utf-8"))?)
            .unwrap_or(0);
    }

    return Ok(total);
}
