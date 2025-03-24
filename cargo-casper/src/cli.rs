use std::{
    io,
    path::{Path, PathBuf},
};

use clap::Subcommand;
use include_dir::{Dir, DirEntry};

pub mod build;
pub mod build_schema;
pub mod new;

/// Recursively extracts a virtual (embedded) directory into the specified path.
fn extract_dir(dir: &Dir, target: &Path) -> io::Result<()> {
    // Ensure the target directory exists.
    std::fs::create_dir_all(target)?;

    // Iterate over each entry in the directory.
    for entry in dir.entries() {
        match entry {
            DirEntry::File(file) => {
                let file_path = target.join(file.path());
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(file_path, file.contents())?;
            }
            DirEntry::Dir(sub_dir) => {
                extract_dir(sub_dir, &target)?;
            }
        }
    }
    Ok(())
}

/// Writes the binary-embedded into a filesystem directory.
/// Returns the path to the extracted dir.
pub(crate) fn extract_embedded_dir(
    extract_to: &Path,
    virtual_dir: &Dir,
) -> std::io::Result<PathBuf> {
    extract_dir(virtual_dir, &extract_to)?;
    Ok(extract_to.into())
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Build the JSON schema of the contract.
    BuildSchema {
        /// Where should the build artifacts be saved?
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Build the contract with its JSON schema embedded.
    Build {
        /// Where should the build artifacts be saved?
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Should the schema be embedded and exposed in the contract? (Default: true)
        #[arg(short, long)]
        embed_schema: Option<bool>,
    },
    /// Creates a new VM2 smart contract project from a template.
    New {
        /// Name of the project to create
        name: String,
    },
}

#[derive(Debug, clap::Parser)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,
}
