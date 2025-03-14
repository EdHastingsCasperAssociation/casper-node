use std::path::PathBuf;

use clap::Subcommand;

pub mod build;
pub mod build_schema;

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    BuildSchema {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        manifest: clap_cargo::Manifest,
        #[command(flatten)]
        workspace: clap_cargo::Workspace,
        #[command(flatten)]
        features: clap_cargo::Features,
    },
    Build {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        embed_schema: Option<bool>,
        #[command(flatten)]
        manifest: clap_cargo::Manifest,
        #[command(flatten)]
        workspace: clap_cargo::Workspace,
        #[command(flatten)]
        features: clap_cargo::Features,
    }
}

#[derive(Debug, clap::Parser)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,
}