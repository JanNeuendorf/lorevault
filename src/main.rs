mod cli;
mod config;
mod memfolder;
mod sources;
use anyhow::{format_err, Context, Error, Result};
use clap::Parser;
use cli::{get_confirmation, Cli, Commands};
use colored::*;
use config::Config;
use memfolder::MemFolder;
use sources::{compute_hash, fetch_first_valid, FileSource};
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    process::exit,
};

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Sync {
            output,
            file,
            tags,
            no_confirm,
        } => sync_folder(output, file, tags, *no_confirm),
        Commands::Check { file } => check(file),
        Commands::Example {} => write_example_config(),
        Commands::Hash { file } => print_hash(file),
    };
    if let Err(e) = result {
        let warning = format!("Error: {}", e);
        println!("{}", warning.red());
        exit(1)
    }
}

fn sync_folder(
    output: &PathBuf,
    config_path: &str,
    tags: &Vec<String>,
    no_confirm: bool,
) -> Result<()> {
    let conf = Config::from_general_path(config_path)?;
    let reference = MemFolder::load_from_folder(output).unwrap_or(MemFolder::empty());
    let memfolder = MemFolder::load_first_valid_with_ref(&conf, tags, &reference)?;

    if !no_confirm && output.exists() && !get_confirmation(output, memfolder.0.keys().count()) {
        return Err(format_err!("Folder overwrite not confirmed."));
    }

    memfolder.write_to_folder(output)?;
    Ok(())
}

fn check(config_path: &str) -> Result<()> {
    let conf = Config::from_general_path(config_path)?;
    let number_of_sources = conf.get_all().iter().map(|f| &f.sources).flatten().count();
    let mut source_counter = 0;
    for file in conf.get_all() {
        if file.hash.is_none() {
            let warning = format!(
                "No hash for {}",
                display_filename(file.get_path(), &file.get_tags())
            );
            println!("{}", warning.yellow())
        } else {
            println!(
                "working on {}",
                display_filename(file.get_path(), &file.get_tags())
            );
        }
        let mut working_hash = file.hash.clone();
        let mut misses = 0;
        for source in &file.sources {
            source_counter += 1;
            match &source.fetch() {
                Ok(contents) => {
                    let current_hash = compute_hash(&contents);
                    match &working_hash {
                        Some(h) => {
                            if h != &current_hash {
                                return Err(format_err!(
                                    "Hash did not match for {}",
                                    display_filename(file.get_path(), &file.get_tags())
                                ));
                            }
                        }
                        None => working_hash = Some(current_hash),
                    }
                    let msg = format!(
                        "Checked {}/{} : {:?}",
                        source_counter, number_of_sources, source
                    );
                    println!("{}", msg.green());
                }
                Err(e) => {
                    let warning = format!(
                        "Failed {}/{} : {:?}",
                        source_counter, number_of_sources, source
                    );
                    println!("{}", warning.yellow());
                    println!("{}", format!("{}", e).yellow());
                    misses = misses + 1;
                    if misses == file.sources.len() {
                        return Err(format_err!(
                            "No valid sources for {}",
                            display_filename(file.get_path(), &file.get_tags())
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn display_filename(path: &PathBuf, tags: &Vec<String>) -> String {
    if tags.is_empty() {
        format!("path={} ", path.to_string_lossy())
    } else {
        format!("path={} / tags={}", path.to_string_lossy(), tags.join(", "))
    }
}

fn write_example_config() -> Result<()> {
    let conf = include_str!("lorevault_example.toml");
    if PathBuf::from("lorevault_example.toml").exists() {
        return Err(format_err!("lorevault_example.toml already exists."));
    }
    fs::write("lorevault_example.toml", conf)?;
    Ok(())
}

fn print_hash(path: &str) -> Result<()> {
    let content = fs::read(path)?;
    let hash = compute_hash(&content);
    println!("hash= \"{}\"", hash);
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_example_config() {
        let _conf: Config = toml::from_str(include_str!("lorevault_example.toml")).unwrap();
    }
}
