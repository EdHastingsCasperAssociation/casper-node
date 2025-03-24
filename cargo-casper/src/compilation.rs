use std::{
    ffi::OsStr,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "reason")]
enum CargoMessage {
    #[serde(rename = "compiler-artifact")]
    CompilerArtifact { filenames: Vec<String> },
    #[serde(other)]
    Other,
}

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

    /// Dispatches the compilation job. This builds the Cargo project into a temporary target
    /// directory.
    pub fn dispatch<T, I, S>(&self, target: T, extra_features: I) -> Result<CompilationResults>
    where
        T: Into<String>,
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let target: String = target.into();

        // Merge the configured features with any extra features
        let mut features = self.features.clone();
        features.extend(extra_features.into_iter().map(Into::into));
        let features_str = features.join(",");

        let build_args = [
            "build",
            "--manifest-path",
            self.manifest_path,
            "--target",
            target.as_str(),
            "--features",
            &features_str,
            "--lib",
            "--release",
            "--message-format=json",
        ];

        // Get any rustflags from the environment and combine with the additional rustflags
        let env_rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();

        let rustflags = match self.rustflags.as_ref() {
            Some(additional) if env_rustflags.is_empty() => additional.clone(),
            Some(additional) => format!("{} {}", env_rustflags, additional),
            None => env_rustflags,
        };

        // Run the cargo build command and capture the output
        let mut handle = Command::new("cargo")
            .args(&build_args)
            .env("RUSTFLAGS", rustflags)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed spawning child process")?;

        let stdout = handle.stdout.take().unwrap();
        let reader = BufReader::new(stdout);

        let mut artifacts = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(msg) = serde_json::from_str::<CargoMessage>(&line) {
                if let CargoMessage::CompilerArtifact { filenames } = msg {
                    for artifact in &filenames {
                        let path = PathBuf::from(artifact);
                        if path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(OsStr::to_str)
                            != Some("deps")
                        {
                            artifacts.push(PathBuf::from(artifact));
                        }
                    }
                }
            }
        }

        let output = handle
            .wait_with_output()
            .context("Failed compiling user wasm")?;

        if !output.status.success() {
            return Err(anyhow!(
                "Cargo build failed:\nSTDERR: {}\nSTDOUT: {}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ));
        }

        Ok(CompilationResults { artifacts })
    }
}

/// Results of a compilation job.
pub(crate) struct CompilationResults {
    artifacts: Vec<PathBuf>,
}

impl CompilationResults {
    /// Returns a slice of paths to the build artifacts.
    pub fn artifacts(&self) -> &[PathBuf] {
        &self.artifacts
    }

    pub fn get_artifact_by_extension(&self, extension: &str) -> Option<PathBuf> {
        self.artifacts()
            .iter()
            .filter(|x| x.extension().map(|y| y.to_str()).flatten() == Some(extension))
            .next()
            .map(|x| x.into())
    }
}
