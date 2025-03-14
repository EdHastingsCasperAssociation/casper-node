use std::{
    path::PathBuf,
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;

/// Represents a job to compile a Cargo project.
pub(crate) struct CompileJob<'a> {
    manifest_path: &'a str,
    features: Vec<String>,
    rustflags: Option<String>,
}

impl<'a> CompileJob<'a> {
    /// Creates a new compile job with the given manifest path, optional features,
    /// and optional *additional* rustflags.
    pub fn new(
        manifest_path: &'a str,
        features: Option<Vec<String>>,
        rustflags: Option<String>,
    ) -> Self {
        Self {
            manifest_path,
            features: features.unwrap_or_default(),
            rustflags,
        }
    }

    /// Adds or replaces the additional rustflags for the compilation.
    pub fn with_rustflags(mut self, rustflags: String) -> Self {
        self.rustflags = Some(rustflags);
        self
    }

    /// Dispatches the compilation job. This builds the Cargo project into a temporary target directory.
    pub fn dispatch<T, I, S>(
        &self,
        target: T,
        extra_features: I,
    ) -> Result<CompilationResults>
    where
        T: Into<String>,
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let target: String = target.into();
        let temp_dir = TempDir::new()
            .context("Failed to create temporary directory")?;

        // Merge the configured features with any extra features
        let mut features = self.features.clone();
        features.extend(extra_features.into_iter().map(Into::into));
        let features_str = features.join(",");

        // Build the argument list
        let target_dir = temp_dir
            .path()
            .to_str()
            .context("Temporary directory path is not valid UTF-8")?;

        let args = [
            "build",
            "--manifest-path",
            self.manifest_path,
            "--target",
            target.as_str(),
            "--features",
            &features_str,
            "--lib",
            "--release",
            "--target-dir",
            target_dir,
        ];

        eprintln!("Running cargo with args: {:?}", args);

        // Get any rustflags from the environment and combine with the additional rustflags
        let env_rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();

        let rustflags = match self.rustflags.as_ref() {
            Some(additional) if env_rustflags.is_empty() => additional.clone(),
            Some(additional) => format!("{} {}", env_rustflags, additional),
            None => env_rustflags,
        };

        // Run the cargo build command and capture the output
        let output = Command::new("cargo")
            .args(&args)
            .env("RUSTFLAGS", rustflags)
            .output()
            .context("Failed to execute cargo build command")?;

        if !output.status.success() {
            return Err(anyhow!(
                "Cargo build failed:\nSTDERR: {}\nSTDOUT: {}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ));
        }

        // Determine where the build artifacts are located and read them
        // into a vector
        let artifact_dir = temp_dir.path().join(target).join("release");
        eprintln!("Artifact directory: {:?}", artifact_dir);

        let artifacts = std::fs::read_dir(&artifact_dir)
            .with_context(|| format!("Failed to read artifact directory: {:?}", artifact_dir))?
            .filter_map(|entry| match entry {
                Ok(entry) if entry.path().is_file() => Some(entry.path()),
                _ => None,
            })
            .collect();

        Ok(CompilationResults {
            artifacts,
            temp_dir: Some(temp_dir),
        })
    }
}

/// Results of a compilation job.
pub(crate) struct CompilationResults {
    artifacts: Vec<PathBuf>,
    // Keeps the temporary directory alive so that artifacts remain accessible
    #[allow(dead_code)]
    temp_dir: Option<TempDir>,
}

impl CompilationResults {
    /// Returns a slice of paths to the build artifacts.
    pub fn artifacts(&self) -> &[PathBuf] {
        &self.artifacts
    }
}
