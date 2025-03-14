use std::{fs::File, io::Write};

use clap::Parser;
use cli::{Cli, Command};

pub(crate) mod compilation;
pub(crate) mod injector;
pub(crate) mod cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::BuildSchema { 
            output,
            features,
            ..
        } => {
            // If user specified an output path, write there.
            // Otherwise print to standard output.
            let mut schema_writer: Box<dyn Write> = match output {
                Some(path) => Box::new(File::create(path)?),
                None => Box::new(std::io::stdout()),
            };

            cli::build_schema::build_schema_impl(
                &mut schema_writer,
                features
            )?
        },
        Command::Build { 
            output,
            features,
            ..
        } => {
            cli::build::build_impl(
                output,
                features
            )?
        },
    }
    Ok(())
}
