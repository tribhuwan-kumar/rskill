use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustProject {
    pub path: PathBuf,
    pub name: String,
    pub target_dir: Option<PathBuf>,
    pub target_size: u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub workspace_root: bool,
    pub has_lock_file: bool,
    pub dependencies_count: usize,
    pub build_artifacts: Vec<BuildArtifact>,
    pub cargo_cache_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildArtifact {
    pub path: PathBuf,
    pub artifact_type: ArtifactType,
    pub size: u64,
    pub last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    Target,
    IncrementalCompilation,
    Dependencies,
    Examples,
    Tests,
    Benchmarks,
    CargoRegistry,
    CargoGitCache,
    CargoConfigCache,
}

impl RustProject {
    pub fn total_cleanable_size(&self) -> u64 {
        self.target_size + self.cargo_cache_size
    }

    pub fn format_size(&self, use_gb: bool) -> String {
        let size = self.total_cleanable_size();
        if use_gb {
            format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        } else {
            format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
        }
    }

    pub fn days_since_modified(&self) -> Option<i64> {
        self.last_modified.map(|dt| {
            let now = Utc::now();
            (now - dt).num_days()
        })
    }

    pub fn is_likely_active(&self) -> bool {
        self.days_since_modified()
            .map(|days| days < 30) // Modified within last 30 days
            .unwrap_or(true) // If we can't determine, assume active for safety
    }
}

impl ArtifactType {
    pub fn _description(&self) -> &'static str {
        match self {
            ArtifactType::Target => "Target directory (build outputs)",
            ArtifactType::IncrementalCompilation => "Incremental compilation cache",
            ArtifactType::Dependencies => "Compiled dependencies",
            ArtifactType::Examples => "Compiled examples",
            ArtifactType::Tests => "Compiled tests",
            ArtifactType::Benchmarks => "Compiled benchmarks",
            ArtifactType::CargoRegistry => "Cargo registry cache",
            ArtifactType::CargoGitCache => "Cargo git cache",
            ArtifactType::CargoConfigCache => "Cargo configuration cache",
        }
    }

    pub fn _is_safe_to_delete(&self) -> bool {
        match self {
            ArtifactType::Target
            | ArtifactType::IncrementalCompilation
            | ArtifactType::Dependencies
            | ArtifactType::Examples
            | ArtifactType::Tests
            | ArtifactType::Benchmarks => true,
            ArtifactType::CargoRegistry
            | ArtifactType::CargoGitCache
            | ArtifactType::CargoConfigCache => false, // More global, need warning
        }
    }
}
