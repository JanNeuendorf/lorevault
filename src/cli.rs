use crate::*;
use clap::{Parser, Subcommand};
use regex::Regex;

#[derive(Parser, Debug)]
#[command(version, about, long_about =Some("Make a folder reproducible by specifying its contents in a file."))]
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
        #[arg(help = "Destination folder")]
        output: PathBuf,
        #[arg(short, long)]
        tags: Vec<String>,
        #[arg(
            long,
            default_value = "false",
            help = "Overwrite target folder without confirmation"
        )]
        no_confirm: bool,
    },
    #[command(
        about = "Print a report.",
        long_about = "Print a report on the config. Fails if a file has no valid sources or a hash does not match"
    )]
    Check {
        #[arg(help = "Config file")]
        file: String,
    },
    #[command(about = "Writes out an example file")]
    Example {},
    #[command(about = "Prints the sha3-256 hash of a file.")]
    Hash { file: String },
}

fn is_repo(general_path: &str) -> bool {
    let re = Regex::new(r".*#[0-9a-fA-F]{7,40}:.*").expect("regular expression pattern invalid");

    re.is_match(general_path)
}

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

pub fn source_from_string(general_path: &str) -> Result<sources::FileSource> {
    if is_repo(general_path) {
        match extract_components(general_path) {
            Some((repo, commit, path)) => Ok(sources::FileSource::Git {
                repo: repo.into(),
                commit: commit.into(),
                path: path.into(),
            }),
            None => Err(format_err!("could not parse repo string")),
        }
    } else {
        Ok(sources::FileSource::Local {
            path: general_path.into(),
        })
    }
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
