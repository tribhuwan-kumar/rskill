use std::path::PathBuf;
use clap::{Parser, ValueEnum};

#[derive(Parser, Clone, Debug)]
#[command(
    name = "rskill",
    about = "Find and clean Rust project build artifacts and caches",
    version = "0.3.3"
)]
pub struct Cli {
    /// directory to start searching from current working directory
    #[arg(short, long, default_value = ".")]
    pub directory: PathBuf,

    /// search from user's home directory
    #[arg(short = 'f', long)]
    pub full: bool,

    /// target directories to search for (default: target)
    #[arg(short, long, default_value = "target")]
    pub target: String,

    /// sort results by size, path, or last modified
    #[arg(short, long, value_enum, default_value = "size")]
    pub sort: SortBy,

    /// show sizes in gigabytes instead of megabytes
    #[arg(long)]
    pub gb: bool,

    /// exclude directories from search (comma-separated)
    #[arg(short = 'E', long)]
    pub exclude: Option<String>,

    /// exclude hidden directories
    #[arg(short = 'x', long)]
    pub exclude_hidden: bool,

    /// hide errors
    #[arg(short = 'e', long)]
    pub hide_errors: bool,

    /// automatically delete all found directories
    #[arg(short = 'D', long)]
    pub delete_all: bool,

    /// dry run - don't actually delete anything
    #[arg(long)]
    pub dry_run: bool,

    /// just list projects without interactive mode
    #[arg(short, long)]
    pub list_only: bool,

    /// show additional Rust-specific directories (registry cache, git cache, etc.)
    #[arg(long)]
    pub include_cargo_cache: bool,

    /// don't check for updates
    #[arg(long)]
    pub no_check_update: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum SortBy {
    Size,
    Path,
    LastMod,
}

impl Cli {
    pub fn get_search_directory(&self) -> PathBuf {
        if self.full {
            dirs::home_dir().expect("Failed to get home directory")
        } else {
            self.directory.clone()
        }
    }

    pub fn get_excluded_dirs(&self) -> Vec<String> {
        self.exclude
            .as_ref()
            .map(|s| {
                s.split(',')
                    .map(|dir| dir.trim().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}
