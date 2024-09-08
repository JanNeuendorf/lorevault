//------------------------------------------------------------
//External dependencies
//------------------------------------------------------------
use anyhow::{format_err, Context, Error, Result};
use auth_git2::GitAuthenticator;
use clap::{Parser, Subcommand};
use colored::*;
use ctrlc;
use dialoguer::Confirm;
use dirs::config_dir;
use git2::{Oid, Repository};
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sha3::{Digest, Sha3_256};
use ssh2::Session;
use std::{
    collections::{HashMap, HashSet},
    env::consts::OS,
    fmt, fs,
    io::prelude::*,
    net::TcpStream,
    path::PathBuf,
    process::exit,
    time::Duration,
};
use tempfile::TempDir;
use termion::terminal_size;

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
use {cli::*, config::*, directories::*, edits::*, memfolder::*, sources::*, variables::*};

//------------------------------------------------------------
//constants
//------------------------------------------------------------
pub static CACHEDIR: OnceCell<TempDir> = OnceCell::new();

fn main() {
    let cli = Cli::parse();
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
        Commands::Clean {
            output,
            file,
            tags,
            no_confirm,
            skip_first_level,
        } => clean_command(file, output, tags, *skip_first_level, *no_confirm),
        Commands::Config {
            file,
            tags,
            no_confirm,
        } => sync_dotconf(file, tags, *no_confirm),
        Commands::Show { source, output } => show(source, output),
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
        match &cli.command {
            Commands::Show { output: None, .. } => {}
            _ => green("Operation completed"),
        }
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
        if c_output == cwd && !skip_fist {
            return Err(format_err!(
                "This would overwrite your current working directory!"
            ));
        }
    }

    let conf = Config::from_general_path(config_path, true, None)?;

    let memfolder = MemFolder::load_first_valid_with_ref(&conf, tags, &output)?;
    if !skip_fist {
        if !no_confirm && output.exists() && !get_confirmation(output, memfolder.0.keys().count()) {
            return Err(format_err!("Folder overwrite not confirmed."));
        }

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

fn sync_dotconf(config_path: &str, tags: &Vec<String>, no_confirm: bool) -> Result<()> {
    if OS != "linux" {
        return Err(format_err!(
            "Detecting the config-directory is currently only supported on linux."
        ));
    }
    let dotconf = config_dir().context("Could not detect config directory")?;
    sync_folder(&dotconf, config_path, tags, no_confirm, true)
}

fn show(source: &String, output: &Option<PathBuf>) -> Result<()> {
    let content = FileSource::Auto(source.clone()).fetch()?;
    match output {
        None => {
            let text = String::from_utf8(content)?;
            print!("{}", text);
        }
        Some(file) => fs::write(file, content)?,
    }
    Ok(())
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
    let config = Config::from_general_path(configpath, true, None)?;

    let mut tags = config.tags();
    tags.sort();
    break_line();
    for tag in &tags {
        neutral(format!("- {}", tag));
    }
    break_line();
    Ok(())
}

fn get_active_paths(configpath: &str, tags: &Vec<String>) -> Result<Vec<PathBuf>> {
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
    Ok(active_paths)
}

fn print_list(configpath: &str, tags: &Vec<String>) -> Result<()> {
    let active_paths = get_active_paths(configpath, tags)?;
    break_line();
    for path in active_paths {
        neutral(format!("- {}", path.display()));
    }
    break_line();
    Ok(())
}

fn clean_command(
    configpath: &str,
    output: &PathBuf,
    tags: &Vec<String>,
    skip_first: bool,
    no_confirm: bool,
) -> Result<()> {
    if !skip_first {
        if !no_confirm {
            let prompt = format!(
                "This will delete the directory {}",
                output.to_string_lossy(),
            );
            match Confirm::new().with_prompt(prompt).interact() {
                Ok(true) => {}
                _ => return Err(format_err!("Not confirmed")),
            };
        }
        fs::remove_dir_all(output)?;
        return Ok(());
    } else {
        let all_paths = get_active_paths(configpath, tags)?;
        if !all_paths.iter().all(|p| p.is_relative()) {
            return Err(format_err!(
                "List of paths to delete contains absolute path"
            ));
        }
        let to_delete = vecset(vec![all_paths
            .iter()
            .map(|rel| {
                output.join(
                    rel.iter()
                        .next()
                        .expect("Encountered empty path in deletion"),
                )
            })
            .collect::<Vec<_>>()]);
        if !no_confirm {
            let list = to_delete
                .iter()
                .map(|f| format!("- {}", f.display()))
                .collect::<Vec<String>>()
                .join("\n");
            let prompt = format!("The paths:\n{}\nWill be deleted!\nIs that OK?", list);
            match Confirm::new().with_prompt(prompt).report(false).interact() {
                Ok(true) => {}
                _ => return Err(format_err!("Not confirmed")),
            };
        }

        for f in to_delete {
            if !f.exists() {
                yellow(format!("Skipping missing path {}", f.display()));
                continue;
            }
            if f.is_file() {
                _ = fs::remove_file(f)?;
            } else {
                fs::remove_dir_all(f)?;
            }
        }
        Ok(())
    }
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
