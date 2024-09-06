use crate::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about =Some("Make a directory reproducible by specifying its contents in a file."))]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    #[cfg(feature = "debug")]
    #[arg(
        short,
        long,
        default_value = "false",
        help = "Show some additional debug information."
    )]
    pub debug: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Sync to a specified directory")]
    Sync {
        #[arg(help = "Config file", long_help = "Supports repo#id:path")]
        file: String,
        #[arg(help = "Destination directory")]
        output: PathBuf,
        #[arg(
            short,
            long,
            use_value_delimiter(true),
            long_help = "Tags must be defined in the configuration file"
        )]
        tags: Vec<String>,
        #[arg(
            long,
            short = 'S',
            default_value = "false",
            help = "Ignore paths differing at the first level"
        )]
        skip_first_level: bool,
        #[arg(
            long,
            short = 'Y',
            default_value = "false",
            help = "Overwrite target directory without confirmation"
        )]
        no_confirm: bool,
    },
    #[command(about = "Shortcut for syncing to ~/.config with -S")]
    Config {
        #[arg(help = "Config file", long_help = "Supports repo#id:path")]
        file: String,
        #[arg(
            short,
            long,
            use_value_delimiter(true),
            long_help = "Tags must be defined in the configuration file"
        )]
        tags: Vec<String>,
        #[arg(
            long,
            short = 'Y',
            default_value = "false",
            help = "Overwrite target directory without confirmation"
        )]
        no_confirm: bool,
    },
    #[command(about = "Writes out an example configuration file", alias = "init")]
    Example {},
    #[command(about = "Prints the SHA3-256 hash of a file")]
    Hash { file: String },
    #[command(about = "Lists all the tags defined in the file")]
    Tags { file: String },
    #[command(about = "Lists all the files that would be in the directory")]
    List {
        file: String,
        #[arg(
            short,
            long,
            use_value_delimiter(true),
            long_help = "Tags must be defined in the configuration file"
        )]
        tags: Vec<String>,
    },
}

// A "general_path" is a string that might be a path or repo#id:subpath
pub fn is_repo(general_path: &str) -> bool {
    general_path.contains('#') && general_path.contains(':')
}

// Gets (repo,id,subpath) from a general path
// There are certain combinations of : and # in the url,id and path that can not be expressed with this syntax.
pub fn extract_components(s: &str) -> Option<(&str, &str, &str)> {
    let index_of_last_hash = s.chars().enumerate().filter(|(_i, c)| *c == '#').last()?.0;
    let index_of_last_colon = s.chars().enumerate().filter(|(_i, c)| *c == ':').last()?.0;
    if index_of_last_hash + 1 >= index_of_last_colon {
        return None;
    }
    let repo = &s[0..index_of_last_hash];
    let id = &s[index_of_last_hash + 1..index_of_last_colon];
    let path = &s[index_of_last_colon + 1..];
    Some((repo, id, path))
}

// Takes a general path and tries to parse it into a filesource.
// URLs are not supported
// The reason for this is the added complexity with the SELF_ variables. It is probably not a common usecase.
// It is called simple because sources for files defined in the file are parsed in a similar way,
// but the function for config-files is more conservative.
pub fn source_from_string_simple(general_path: &str) -> Result<sources::FileSource> {
    if is_repo(general_path) {
        match extract_components(general_path) {
            Some((repo, id, path)) => Ok(sources::FileSource::Git {
                repo: repo.into(),
                id: id.into(),
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
    let status = match Confirm::new().with_prompt(prompt).interact() {
        Ok(true) => true,
        _ => false,
    };

    status
}

pub fn get_confirmation_skip_level(folder_path: &PathBuf, tracked_paths: &Vec<PathBuf>) -> bool {
    let file_count = count_files_recursively(folder_path);
    if file_count.is_err() {
        return false;
    }
    let list = tracked_paths
        .iter()
        .map(|f| format!("- {}", f.display()))
        .collect::<Vec<String>>()
        .join("\n");
    let prompt = format!(
        "All paths starting with:\n{}\nWill be overwritten!\nIs that OK?",
        list
    );
    let status = match Confirm::new().with_prompt(prompt).report(false).interact() {
        Ok(true) => true,
        _ => false,
    };

    status
}

// This ignores things that are not files.
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
        assert_eq!(
            extract_components("internet://adress#hash#release:tag:/path.txt"),
            Some(("internet://adress#hash", "release:tag", "/path.txt".into()))
        );
        assert_eq!(extract_components("r#t:p"), Some(("r", "t", "p".into())));
    }
}
