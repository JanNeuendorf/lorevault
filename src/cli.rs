use crate::*;
use clap::{Parser, Subcommand};
use dialoguer::Confirm;
use regex::Regex;

#[derive(Parser, Debug)]
#[command(version, about, long_about =Some("Make a directory reproducible by specifying its contents in a file."))]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Sync to a specified directory")]
    Sync {
        #[arg(help = "Config file", long_help = "Supports repo#commit:path")]
        file: String,
        #[arg(help = "Destination directory")]
        output: PathBuf,
        #[arg(short, long)]
        tags: Vec<String>,
        #[arg(
            long,
            default_value = "false",
            help = "Overwrite target directory without confirmation"
        )]
        no_confirm: bool,
    },
    #[command(
        about = "Print a report",
        long_about = "Prints a report on the config. Fails if a file has no valid sources or a hash does not match"
    )]
    Check {
        #[arg(help = "Config file")]
        file: String,
    },
    #[command(about = "Writes out an example configuration file", alias = "init")]
    Example {},
    #[command(about = "Prints the SHA3-256 hash of a file")]
    Hash { file: String },
    #[command(about = "Lists all the tags defined in the file")]
    Tags { file: String },
    #[command(about = "Lists all the files that would be in the directory.")]
    List {
        file: String,
        #[arg(short, long)]
        tags: Vec<String>,
    },
}

// A "general_path" is a string that might be a path or repo#commit:subpath
fn is_repo(general_path: &str) -> bool {
    let re = Regex::new(r".*#[0-9a-fA-F]{7,40}:.*").expect("regular expression pattern invalid");
    re.is_match(general_path)
}

// Gets (repo,commit,subpath) from a general path
fn extract_components(s: &str) -> Option<(String, String, String)> {
    let re =
        Regex::new(r"(.*?)#([0-9a-fA-F]{7,40}):(.*)").expect("regular expression pattern invalid");

    if let Some(captures) = re.captures(s) {
        let before_hash = captures.get(1)?.as_str().trim().to_string();
        let hash = captures.get(2)?.as_str().trim().to_string();
        let after_hash = captures.get(3)?.as_str().trim().to_string();

        Some((before_hash, hash, after_hash))
    } else {
        None
    }
}

// Takes a general path and tries to parse it into a filesource.
// It is called simple because it does not support archives. The config can not be loaded from an archive.
// The reason for this is the added complexity with the SELF_ variables. It is probably not a common usecase.
pub fn source_from_string_simple(general_path: &str) -> Result<sources::FileSource> {
    if is_repo(general_path) {
        match extract_components(general_path) {
            Some((repo, commit, path)) => Ok(sources::FileSource::Git {
                repo: repo.into(),
                commit: commit.into(),
                path: path.into(),
            }),
            None => Err(format_err!(format!(
                "Could not parse repo string {}",
                general_path
            ))),
        }
    } else {
        Ok(sources::FileSource::Local {
            path: general_path.into(),
        })
    }
}

pub fn get_confirmation(folder_path: &PathBuf, newcount: usize) -> bool {
    let file_count = count_files_recursively(folder_path);
    if file_count.is_err() {
        return false;
    }

    let prompt = format!(
        "Overwrite {} (total {} files) with {} files?",
        folder_path.to_string_lossy(),
        file_count.expect("unchecked file count"),
        newcount
    );
    match Confirm::new().with_prompt(prompt).interact() {
        Ok(true) => true,
        _ => false,
    }
}

fn count_files_recursively(folder_path: &PathBuf) -> Result<usize> {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files_recursively(&path)?;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_repo_string() {
        assert!(is_repo("https://github.com/some/repo.git#fb17a46eb92e8d779e57a10589e9012e9aa5f948:local/path.txt"));
        assert_eq!(extract_components("https://github.com/some/repo.git#fb17a46eb92e8d779e57a10589e9012e9aa5f948:local/path.txt"),
        Some(("https://github.com/some/repo.git".into(),"fb17a46eb92e8d779e57a10589e9012e9aa5f948".into(),"local/path.txt".into())));
        assert!(!is_repo("https://github.com/some/repo.git:local/path.txt"));
        assert!(!is_repo("/home/somefile.toml"));
    }
}
