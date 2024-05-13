//External dependencies
//------------------------------------------------------------
use anyhow::{format_err, Context, Error, Result};
use auth_git2::GitAuthenticator;
use clap::{Parser, Subcommand};
use colored::*;
use ctrlc;
use dialoguer::Confirm;
use git2::{Oid, Repository};
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sha3::{Digest, Sha3_256};
use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    path::PathBuf,
    process::exit,
    time::Duration,
};
use tempfile::TempDir;
use termion::terminal_size;
// Tracing is only used with the debug feature
#[cfg(feature = "debug")]
use tracing::Level;
#[cfg(feature = "debug")]
use tracing_subscriber::FmtSubscriber;
//------------------------------------------------------------
//Internal dependencies
//------------------------------------------------------------
mod cli;
mod config;
mod directories;
mod edits;
mod memfolder;
mod sources;
mod variables;
use cli::*;
use config::{check_recursion, Config, File, Inclusion};
use directories::*;
use edits::*;
use memfolder::MemFolder;
use sources::*;
use variables::*;
//------------------------------------------------------------
//constants
//------------------------------------------------------------
pub static CACHEDIR: OnceCell<TempDir> = OnceCell::new();
const INCLUSION_RECURSION_LIMIT: usize = 10; // The depth of inclusions of other config files.
                                             //------------------------------------------------------------

//info!() does nothing if --features=debug is not active.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        #[cfg(feature = "debug")]
        tracing::info!($($arg)*);
    }};
}

fn main() {
    let cli = Cli::parse();
    #[cfg(feature = "debug")]
    if cli.debug {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Could not initialize output for verbose mode.");
        info!("{:?}", &cli);
    }
    ctrlc::set_handler(move || {
        if let Err(_) = clean_cache_dir() {
            red("Canceled. Cache directory could not be cleaned up");
        } else {
            red("Canceled");
        }
        exit(2);
    })
    .expect("Error setting Ctrl-C handler");

    let result = match &cli.command {
        Commands::Sync {
            output,
            file,
            tags,
            no_confirm,
            skip_first_level,
        } => sync_folder(output, file, tags, *no_confirm, *skip_first_level),
        Commands::Example {} => write_example_config(),
        Commands::Hash { file } => print_hash(file),
        Commands::Tags { file } => print_tags(file),
        Commands::List { file, tags } => print_list(file, tags),
    };
    if let Err(_) = clean_cache_dir() {
        yellow("Cache directory could not be cleaned up");
    }
    if let Err(e) = result {
        red(format!("Error: {}", e));
        exit(1)
    } else {
        green("Operation completed")
    }
}

fn sync_folder(
    output: &PathBuf,
    config_path: &str,
    tags: &Vec<String>,
    no_confirm: bool,
    skip_fist: bool,
) -> Result<()> {
    if let (Ok(c_output), Ok(cwd)) = (output.canonicalize(), std::env::current_dir()) {
        if c_output == cwd {
            return Err(format_err!(
                "This would overwrite your current working directory!"
            ));
        }
    }
    info!(
        "Want to load config from {:?}",
        cli::source_from_string_simple(config_path)
    );
    info!("Checking for recursion");
    check_recursion(config_path)?;
    info!("No recursion found");
    let conf = Config::from_general_path(config_path, true, None)?;
    info!("Parsed config file");
    let memfolder = MemFolder::load_first_valid_with_ref(&conf, tags, &output)?;
    if !skip_fist {
        if !no_confirm && output.exists() && !get_confirmation(output, memfolder.0.keys().count()) {
            return Err(format_err!("Folder overwrite not confirmed."));
        }
        info!("Trying to create folder");
        memfolder.write_to_folder(output)?;
        Ok(())
    } else {
        let tracked = memfolder.tracked_subpaths()?;
        if !no_confirm && output.exists() && !get_confirmation_skip_level(output, &tracked) {
            return Err(format_err!("Folder overwrite not confirmed."));
        }
        memfolder.write_to_folder_skip_first(output)?;
        Ok(())
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
    break_line();
    let mut tags = config.tags();
    tags.sort();
    for tag in &tags {
        neutral(format!("- {}", tag));
    }
    break_line();
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
    break_line();
    for path in active_paths {
        neutral(format!("- {}", path.display()));
    }
    break_line();
    Ok(())
}

fn clean_cache_dir() -> Result<()> {
    match CACHEDIR.get() {
        Some(cd) => {
            fs::remove_dir_all(cd.path())?;
            Ok(())
        }
        None => Ok(()),
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
pub fn break_line() {
    let columns = terminal_size().unwrap_or((5, 5)).0;
    println!(
        "{}",
        std::iter::repeat('-')
            .take(columns as usize)
            .collect::<String>()
    );
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_example_config() {
        let _conf: Config = toml::from_str(include_str!("lorevault_example.toml")).unwrap();
    }
}
