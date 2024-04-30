mod cli;
mod config;
mod edits;
mod memfolder;
mod sources;
mod variables;
use anyhow::{format_err, Context, Error, Result};
use clap::Parser;
use cli::{get_confirmation, source_from_string_simple, Cli, Commands};
use colored::*;
use config::{check_recursion, Config, File};
use edits::*;
use memfolder::MemFolder;
use once_cell::sync::OnceCell;
use sources::{compute_hash, format_subpath, is_url, FileSource};
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    process::exit,
};
use tempfile::TempDir;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use variables::*;

pub static CACHEDIR: OnceCell<TempDir> = OnceCell::new();

fn main() {
    let cli = Cli::parse();
    if cli.debug {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Could not initialize output for verbose mode.");
        info!("{:?}", &cli);
    }
    match init_cache_dir() {
        Err(_) => yellow("Not using a cache directory."),
        _ => {
            info!(
                "Cache directory {}",
                CACHEDIR.get().expect("Cachedir lost").path().display()
            );
        }
    }

    let result = match &cli.command {
        Commands::Sync {
            output,
            file,
            tags,
            no_confirm,
        } => sync_folder(output, file, tags, *no_confirm),
        Commands::Check {
            file,
            pedantic: pdedantic,
        } => check(file, *pdedantic),
        Commands::Example {} => write_example_config(),
        Commands::Hash { file } => print_hash(file),
        Commands::Tags { file } => print_tags(file),
        Commands::List { file, tags } => print_list(file, tags),
    };
    if let Err(e) = result {
        red(format!("Error: {}", e));
        exit(1)
    }
}

fn sync_folder(
    output: &PathBuf,
    config_path: &str,
    tags: &Vec<String>,
    no_confirm: bool,
) -> Result<()> {
    check_recursion(config_path)?;
    let conf = Config::from_general_path(config_path, true, None)?;
    info!("Parsed config file");

    let reference = match MemFolder::load_from_folder(output) {
        Ok(r) => {
            info!("Folder already exists. Loaded for reference");
            r
        }
        Err(_) => {
            info!("Folder could not be loaded from reference, starting from scratch");
            MemFolder::empty()
        }
    };
    let memfolder = MemFolder::load_first_valid_with_ref(&conf, tags, &reference)?;

    if !no_confirm && output.exists() && !get_confirmation(output, memfolder.0.keys().count()) {
        return Err(format_err!("Folder overwrite not confirmed."));
    }
    info!("Trying to create folder");
    memfolder.write_to_folder(output)?;
    Ok(())
}

fn check(config_path: &str, pedantic: bool) -> Result<()> {
    check_recursion(config_path)?;

    let conf = Config::from_general_path(config_path, true, None)?;
    if pedantic && !conf.is_fully_hardened()? {
        return Err(format_err!("There are files or inclusions without hashes!"));
    }
    let number_of_sources = conf.get_all()?.iter().map(|f| &f.sources).flatten().count();
    let mut source_counter = 0;
    for file in conf.get_all()? {
        if file.hash.is_none() {
            yellow(format!(
                "No hash for {}",
                display_filename(file.get_path(), &file.get_tags())
            ));
        } else {
            neutral(format!(
                "working on {}",
                display_filename(file.get_path(), &file.get_tags())
            ));
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
                    green(format!(
                        "Checked {}/{} : {:?}",
                        source_counter, number_of_sources, source
                    ));
                }
                Err(e) => {
                    yellow(format!(
                        "Failed {}/{} : {:?}",
                        source_counter, number_of_sources, source
                    ));
                    yellow(format!("{}", e));
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
    green("Saved example as lorevault_example.toml");
    Ok(())
}

fn print_hash(path: &str) -> Result<()> {
    let content = fs::read(path)?;
    let hash = compute_hash(&content);
    neutral(format!("hash = \"{}\"", hash));
    Ok(())
}
fn print_tags(configpath: &str) -> Result<()> {
    check_recursion(configpath)?;
    let config = Config::from_general_path(configpath, true, None)?;
    for tag in &config.tags() {
        neutral(format!("- {}", tag));
    }
    Ok(())
}

fn print_list(configpath: &str, tags: &Vec<String>) -> Result<()> {
    check_recursion(configpath)?;
    let config = Config::from_general_path(configpath, true, None)?;
    let mut active_paths = config
        .get_active(tags)?
        .iter()
        .map(|f| format_subpath(&f.path))
        .collect::<Vec<PathBuf>>();
    active_paths.sort_by(|a, b| {
        let a_components: Vec<_> = a.components().collect();
        let b_components: Vec<_> = b.components().collect();
        let max_len = a_components.len().min(b_components.len());
        for i in 0..max_len {
            if a_components[i] != b_components[i] {
                return a_components[..i].cmp(&b_components[..i]);
            }
        }
        a_components.len().cmp(&b_components.len())
    });
    for path in active_paths {
        neutral(format!("{}", path.display()));
    }
    Ok(())
}

fn init_cache_dir() -> Result<PathBuf> {
    let tmpdir = TempDir::new()?;
    let path = tmpdir.path().to_path_buf();
    let result = CACHEDIR.set(tmpdir);
    match result {
        Ok(_) => Ok(path),
        Err(td) => Err(format_err!("Could not init cachedir {:?}", td)),
    }
}
pub fn yellow(warning: impl AsRef<str>) {
    println!("{}", warning.as_ref().yellow());
}
pub fn red(error: impl AsRef<str>) {
    eprintln!("{}", error.as_ref().red());
}
pub fn green(message: impl AsRef<str>) {
    println!("{}", message.as_ref().green());
}
pub fn neutral(message: impl AsRef<str>) {
    println!("{}", message.as_ref());
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_example_config() {
        let _conf: Config = toml::from_str(include_str!("lorevault_example.toml")).unwrap();
    }
}
