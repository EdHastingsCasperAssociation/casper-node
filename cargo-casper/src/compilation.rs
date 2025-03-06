use std::{path::PathBuf, process::Command};

use anyhow::{bail, Context};
use tempfile::TempDir;

pub(crate) struct CompileJob<'a> {
    package: &'a str,
    features: Option<Vec<String>>,
    rustflags: Option<String>,
}

pub(crate)  struct CompilationResults {
    artifacts: Vec<PathBuf>,
}

impl CompilationResults {
    pub fn artifacts(&self) -> &[PathBuf] {
        &self.artifacts
    }
}

impl<'a> CompileJob<'a> {
    pub fn new(
        package: &'a str,
        features: Option<Vec<String>>,
        rustflags: Option<String>,
    ) -> Self {
        Self {
            package,
            features,
            rustflags,
        }
    }

    pub fn with_rustflags(mut self, rustflags: String) -> Self {
        self.rustflags = Some(rustflags);
        self
    }

    pub fn dispatch<T: IntoIterator<Item = String>>(
        &self,
        target: &'static str,
        extra_features: T
    ) -> Result<CompilationResults, anyhow::Error> {
        let tempdir = TempDir::new()
            .with_context(|| "Failed to create temporary directory")?;

        let mut features = self.features.clone().unwrap_or_default();
        features.extend(extra_features);

        let features_str = features.join(",");

        let mut args = vec!["build", "-p", self.package];
        args.extend(["--target", target]);
        args.extend(["--features", &features_str, "--lib", "--release"]);
        args.extend([
            "--target-dir",
            &tempdir.path().as_os_str().to_str().expect("invalid path"),
        ]);

        eprintln!("Running command {:?}", args);

        let rustflags = if let Some(rustflags) = &self.rustflags {
            rustflags.to_owned()
        } else {
            std::env::var("RUSTFLAGS").unwrap_or_default()
        };

        let mut output = Command::new("cargo")
            .args(&args)
            .env("RUSTFLAGS", rustflags)
            .spawn()
            .with_context(|| "Failed to execute cargo build command")?;

        let exit_status = output
            .wait()
            .with_context(|| "Failed to wait on child")?;

        if !exit_status.success() {
            eprintln!("Command executed with failing error code");
            std::process::exit(exit_status.code().unwrap_or(1));
        }

        let artifact_dir = tempdir.path().join(target).join("release");

        let artifacts: Vec<_> = std::fs::read_dir(&artifact_dir)
            .with_context(|| "Artifact read directory failure")?
            .into_iter()
            .filter_map(|dir_entry| {
                let dir_entry = dir_entry.unwrap();
                let path = dir_entry.path();
                if path.is_file()
                    && dbg!(&path)
                        .extension()?
                        .to_str()
                        .expect("valid string")
                        .ends_with(&std::env::consts::DLL_SUFFIX[1..])
                {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        if artifacts.len() != 1 {
            bail!("Expected exactly one build artifact: {:?}", artifacts);
        }

        Ok(CompilationResults {
            artifacts
        })
    }
}