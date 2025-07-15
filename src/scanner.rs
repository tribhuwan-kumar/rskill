use std::fs;
use tokio::task;
use crate::utils;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;
use colored::Colorize;
use chrono::{DateTime, Utc};
use crate::cli::{Cli, SortBy};
use spinoff::{spinners, Spinner};
use crate::project::{ArtifactType, BuildArtifact, RustProject};

pub struct ProjectScanner {
    cli: Cli,
}

impl ProjectScanner {
    pub fn new(cli: Cli) -> Self {
        Self { cli }
    }

    pub async fn scan(&self) -> Result<Vec<RustProject>> {
        let search_dir = self.cli.get_search_directory();
        let excluded_dirs = self.cli.get_excluded_dirs();

        let spinner = Spinner::new(
            spinners::Dots,
            format!("Scanning for Rust projects in: {}", search_dir.display()),
            spinoff::Color::White,
        );

        let cli_clone = self.cli.clone();
        let projects = task::spawn_blocking(move || {
            Self::find_rust_projects(&search_dir, &excluded_dirs, &cli_clone)
        }).await??;

        spinner.clear();

        Ok(projects)
    }

    fn find_rust_projects(
        search_dir: &Path, 
        excluded_dirs: &[String], 
        cli: &Cli
    ) -> Result<Vec<RustProject>> {
        let mut projects = Vec::new();
        let mut processed_paths = std::collections::HashSet::new();

        for entry in WalkDir::new(search_dir)
            .follow_links(false)
            .max_depth(if cli.full { 10 } else { 5 })
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            // Skip if this is an excluded directory
            if Self::is_excluded_path(path, excluded_dirs, cli.exclude_hidden) {
                continue;
            }

            // Look for Cargo.toml files
            if path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml")) {
                let project_dir = path.parent().unwrap();
                
                // Avoid processing the same project multiple times
                if processed_paths.contains(project_dir) {
                    continue;
                }
                
                processed_paths.insert(project_dir.to_path_buf());
                
                if let Ok(project) = Self::analyze_rust_project(project_dir, cli) {
                    projects.push(project);
                }
            }
        }

        // Sort projects according to CLI preferences
        Self::sort_projects(&mut projects, &cli.sort, cli.gb);
        
        Ok(projects)
    }

    fn is_excluded_path(path: &Path, excluded_dirs: &[String], exclude_hidden: bool) -> bool {
        // Check if any component is in excluded list
        for component in path.components() {
            let comp_str = component.as_os_str().to_string_lossy();
            
            if excluded_dirs.iter().any(|excluded| comp_str.contains(excluded)) {
                return true;
            }
            
            if exclude_hidden && comp_str.starts_with('.') {
                return true;
            }
        }
        
        false
    }

    fn analyze_rust_project(project_dir: &Path, cli: &Cli) -> Result<RustProject> {
        let cargo_toml_path = project_dir.join("Cargo.toml");
        let cargo_lock_path = project_dir.join("Cargo.lock");
        
        // Parse Cargo.toml to get project name and info
        let cargo_toml_content = fs::read_to_string(&cargo_toml_path)?;
        let project_name = Self::extract_project_name(&cargo_toml_content)
            .unwrap_or_else(|| {
                project_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        // Check for target directory
        let target_dir = project_dir.join(&cli.target);
        let (target_size, target_exists) = if target_dir.exists() {
            (utils::calculate_dir_size(&target_dir)?, true)
        } else {
            (0, false)
        };

        // Get last modified time
        let last_modified = Self::get_last_modified_time(project_dir)?;

        // Analyze build artifacts
        let build_artifacts = if target_exists {
            Self::analyze_build_artifacts(&target_dir)?
        } else {
            Vec::new()
        };

        // Calculate cargo cache size if requested
        let cargo_cache_size = if cli.include_cargo_cache {
            Self::calculate_cargo_cache_size()?
        } else {
            0
        };

        // Count dependencies
        let dependencies_count = Self::count_dependencies(&cargo_toml_content);

        Ok(RustProject {
            path: project_dir.to_path_buf(),
            name: project_name,
            target_dir: if target_exists { Some(target_dir) } else { None },
            target_size,
            last_modified,
            workspace_root: Self::is_workspace_root(&cargo_toml_content),
            has_lock_file: cargo_lock_path.exists(),
            dependencies_count,
            build_artifacts,
            cargo_cache_size,
        })
    }

    fn extract_project_name(cargo_toml: &str) -> Option<String> {
        for line in cargo_toml.lines() {
            if line.trim().starts_with("name") {
                if let Some(name_part) = line.split('=').nth(1) {
                    return Some(
                        name_part
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string()
                    );
                }
            }
        }
        None
    }

    fn get_last_modified_time(project_dir: &Path) -> Result<Option<DateTime<Utc>>> {
        let mut latest = None;
        
        // check various files for modification time
        let files_to_check = ["Cargo.toml", "Cargo.lock", "src/main.rs", "src/lib.rs"];
        
        for file in &files_to_check {
            let file_path = project_dir.join(file);
            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(modified) = metadata.modified() {
                    let datetime: DateTime<Utc> = modified.into();
                    latest = Some(latest.map_or(datetime, |prev: DateTime<Utc>| prev.max(datetime)));
                }
            }
        }
        
        Ok(latest)
    }

    fn analyze_build_artifacts(target_dir: &Path) -> Result<Vec<BuildArtifact>> {
        let mut artifacts = Vec::new();
        
        if !target_dir.exists() {
            return Ok(artifacts);
        }

        for entry in WalkDir::new(target_dir).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            
            if path.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                let artifact_type = match dir_name.as_ref() {
                    "debug" | "release" => ArtifactType::Target,
                    "incremental" => ArtifactType::IncrementalCompilation,
                    "deps" => ArtifactType::Dependencies,
                    "examples" => ArtifactType::Examples,
                    _ => continue,
                };
                
                let size = utils::calculate_dir_size(path).unwrap_or(0);
                let last_modified = fs::metadata(path)
                    .and_then(|m| m.modified())
                    .map(DateTime::<Utc>::from)
                    .ok();
                
                artifacts.push(BuildArtifact {
                    path: path.to_path_buf(),
                    artifact_type,
                    size,
                    last_modified,
                });
            }
        }
        
        Ok(artifacts)
    }

    fn calculate_cargo_cache_size() -> Result<u64> {
        let mut total_size = 0u64;
        
        if let Some(home) = dirs::home_dir() {
            let cargo_dir = home.join(".cargo");
            
            // Registry cache
            let registry_dir = cargo_dir.join("registry");
            if registry_dir.exists() {
                total_size += utils::calculate_dir_size(&registry_dir)?;
            }
            
            // Git cache
            let git_dir = cargo_dir.join("git");
            if git_dir.exists() {
                total_size += utils::calculate_dir_size(&git_dir)?;
            }
        }
        
        Ok(total_size)
    }

    fn count_dependencies(cargo_toml: &str) -> usize {
        let mut in_dependencies = false;
        let mut count = 0;
        
        for line in cargo_toml.lines() {
            let trimmed = line.trim();
            
            if trimmed.starts_with('[') {
                in_dependencies = trimmed.starts_with("[dependencies")
                    || trimmed.starts_with("[dev-dependencies")
                    || trimmed.starts_with("[build-dependencies");
                continue;
            }
            
            if in_dependencies && !trimmed.is_empty() && !trimmed.starts_with('#') {
                count += 1;
            }
        }
        
        count
    }

    fn is_workspace_root(cargo_toml: &str) -> bool {
        cargo_toml.contains("[workspace]")
    }

    fn sort_projects(projects: &mut Vec<RustProject>, sort_by: &SortBy, _use_gb: bool) {
        match sort_by {
            SortBy::Size => {
                projects.sort_by(|a, b| b.total_cleanable_size().cmp(&a.total_cleanable_size()));
            }
            SortBy::Path => {
                projects.sort_by(|a, b| a.path.cmp(&b.path));
            }
            SortBy::LastMod => {
                projects.sort_by(|a, b| {
                    match (a.last_modified, b.last_modified) {
                        (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                });
            }
        }
    }

    pub async fn print_projects(&self, projects: &[RustProject]) -> Result<()> {
        if projects.is_empty() {
            print!("No Rust projects found.");
            return Ok(());
        }

        println!(
            "\n{:<30} {:<15} {:<20} {:<15} {:<10}",
            "Project Name".bold(),
            "Size".bold(),
            "Path".bold(),
            "Last Modified".bold(),
            "Status".bold()
        );
        println!("{}", "─".repeat(100));

        for project in projects {
            let size_str = project.format_size(self.cli.gb);
            let path_str = project.path.display().to_string();
            let path_display = if path_str.len() > 18 {
                format!("...{}", &path_str[path_str.len() - 15..])
            } else {
                path_str
            };

            let last_mod = project
                .last_modified
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let status = if project.is_likely_active() {
                "Active".green()
            } else {
                "Stale".yellow()
            };

            let warning = if !project.is_likely_active() && project.total_cleanable_size() == 0 {
                ""
            } else if !project.target_dir.is_some() {
                " (no target)"
            } else {
                ""
            };

            println!(
                "{:<30} {:<15} {:<20} {:<15} {:<10}{}",
                project.name,
                size_str.cyan(),
                path_display,
                last_mod,
                status,
                warning.red()
            );
        }

        let total_size: u64 = projects.iter().map(|p| p.total_cleanable_size()).sum();
        let total_size_str = if self.cli.gb {
            format!("{:.2} GB", total_size as f64 / (1024.0 * 1024.0 * 1024.0))
        } else {
            format!("{:.2} MB", total_size as f64 / (1024.0 * 1024.0))
        };

        println!("\nTotal cleanable space: {}", total_size_str.bold().green());
        
        Ok(())
    }
}
