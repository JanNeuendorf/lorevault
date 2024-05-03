use std::fmt::{self};

use crate::*;

use auth_git2::GitAuthenticator;
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use tar::Archive;
use tempfile::TempDir;
use xz2::read::XzDecoder;
use zip::read::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum FileSource {
    #[serde(rename = "file", alias = "local")]
    Local { path: PathBuf },
    #[serde(rename = "http")]
    Download { url: String },
    #[serde(rename = "git")]
    Git {
        repo: String,
        id: String,
        path: PathBuf,
    },
    #[serde(rename = "text")]
    Text {
        content: String,
        #[serde(default)]
        ignore_variables: bool,
    },
    #[serde(rename = "archive")]
    Archive { archive: PathBuf, path: PathBuf },
    #[serde(untagged)]
    Auto(String),
}

impl fmt::Display for FileSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileSource::Local { path } => write!(f, "{}", path.display()),
            FileSource::Download { url } => write!(f, "{}", url),
            FileSource::Git { repo, id, path } => write!(f, "{}#{}:{}", repo, id, path.display()),
            FileSource::Text { .. } => write!(f, "Custom text"),
            FileSource::Archive { archive, path } => {
                write!(f, "{}:{}", archive.display(), path.display())
            }
            FileSource::Auto(a) => write!(f, "{}", a),
        }
    }
}

impl FileSource {
    pub fn fetch(&self) -> Result<Vec<u8>> {
        match self {
            FileSource::Auto(auto) => parse_auto_source(auto)?.fetch(),
            FileSource::Local { path } => {
                if path.is_relative() {
                    return Err(format_err!(
                        "Relative paths are not allowed: {}",
                        path.to_string_lossy()
                    ));
                }
                fs::read(path).context(format!(
                    "Could not read local file {}",
                    path.to_string_lossy()
                ))
            }
            FileSource::Download { url } => {
                let response = reqwest::blocking::get(url)?;
                let bytes = response.error_for_status()?.bytes()?.to_vec();
                Ok(bytes)
            }
            FileSource::Git {
                repo,
                id: commit,
                path,
            } => get_git_file(commit, path, repo),
            FileSource::Text { content, .. } => Ok(content.clone().into_bytes()),
            FileSource::Archive { archive, path } => {
                if archive.is_relative() {
                    return Err(format_err!(
                        "Relative paths to archives are not allowed: {}",
                        path.to_string_lossy()
                    ));
                }
                let filename = archive
                    .file_name()
                    .context("Archives need endings (.zip,..)")?
                    .to_string_lossy();
                if filename.ends_with(".zip") {
                    extract_file_from_zip(archive, path)
                } else if filename.ends_with(".tar") {
                    extract_file_from_tar(archive, path)
                } else if filename.ends_with(".tar.xz") {
                    extract_file_from_xz_tar(archive, path)
                } else {
                    Err(format_err!(
                        "Unsupported archive type: {}",
                        archive.to_string_lossy()
                    ))
                }
            }
        }
    }
}

pub fn compute_hash(content: &Vec<u8>) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(content);

    let result = hasher.finalize();
    let hex_string: String = result
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join("");
    return hex_string;
}

fn get_git_file(id: &str, file_path: &PathBuf, repo_path: &str) -> Result<Vec<u8>> {
    let repo = get_git_repo(repo_path)?;
    let commit_hash = get_commit_from_string(&repo, id)?;

    let commit = repo.find_commit(Oid::from_str(&commit_hash)?)?;
    let tree = commit.tree()?;

    let blob = tree
        .get_path(&std::path::Path::new(&format_subpath(file_path)))?
        .to_object(&repo)?;

    if let Some(blob) = blob.as_blob() {
        Ok(blob.content().to_vec())
    } else {
        Err(format_err!(
            "Git object is not a blob {}:{}",
            repo_path,
            file_path.to_string_lossy()
        ))
    }
}

fn get_commit_from_string(repo: &Repository, input: &str) -> Result<String> {
    let obj = repo
        .revparse_single(input.trim())
        .context(format!("Could not find commit for id: {}", input))?;
    if let Some(commit) = obj.as_commit() {
        let commit_string = commit.id().to_string();
        info!("ID {} matched to commit {}", input, commit_string);
        return Ok(commit_string);
    }

    Err(format_err!("Could not find commit for id: {}", input))
}

fn get_git_repo(repo_path: &str) -> Result<Repository> {
    let repo: Repository;
    if is_url_or_ssh(repo_path) {
        repo = match fetch_repo_from_cache(repo_path) {
            Ok(r) => r,
            Err(_) => {
                green(format!("Cloning {}", repo_path));
                clone_repository(repo_path)?
            }
        };
    } else {
        if PathBuf::from(repo_path).is_relative() {
            return Err(format_err!("Relative paths are not allowed: {}", repo_path));
        }

        repo = Repository::open(repo_path)?;
    }
    Ok(repo)
}

pub fn is_url_or_ssh(path: &str) -> bool {
    path.to_string().starts_with("http://")
        || path.to_string().starts_with("https://")
        || (path.contains('@') && path.contains(':'))
}

fn cache_name(url: impl AsRef<str>) -> PathBuf {
    PathBuf::from(compute_hash(&url.as_ref().bytes().collect()))
}

fn clone_repository(repo_url: &str) -> Result<Repository> {
    let auth = GitAuthenticator::default();
    if let Some(cachedir) = CACHEDIR.get() {
        let repo_at_cache = auth.clone_repo(
            repo_url,
            cachedir.path().join(cache_name(repo_url)).as_path(),
        );
        let repo = match repo_at_cache {
            Ok(repo) => repo,
            Err(_) => {
                let temp_dir = TempDir::new()?;
                yellow("Trying to write to existing cache directory");
                auth.clone_repo(repo_url, temp_dir.path())?
            }
        };

        Ok(repo)
    } else {
        let temp_dir = TempDir::new()?;
        yellow("Cloning without caching.");

        let repo = auth.clone_repo(repo_url, temp_dir.path())?;

        Ok(repo)
    }
}

fn get_remote_url(repo_path: &PathBuf) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let remote_name = "origin";
    let remote = repo.find_remote(&remote_name)?;

    if let Some(url) = remote.url() {
        Ok(url.to_string())
    } else {
        Err(format_err!("Remote URL not found"))
    }
}

fn fetch_repo_from_cache(url: &str) -> Result<Repository> {
    let cachedir = CACHEDIR.get().context("No cache directory")?.path();
    let path = cachedir.join(cache_name(url));

    if let Ok(found_url) = get_remote_url(&path) {
        if found_url == url {
            return Ok(Repository::open(path)?);
        }
    }
    Err(format_err!("Not found in cache {}", url))
}

fn extract_file_from_zip(path_to_zip: &PathBuf, sub_path: &PathBuf) -> Result<Vec<u8>> {
    let zip_data = fs::read(path_to_zip)?;

    let reader = Cursor::new(zip_data);
    let mut zip = ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;

        let entry_subpath = strip_first_level(entry.name());
        if entry_subpath == format_subpath(sub_path).to_str().context("invalid path")? {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            return Ok(content);
        }
    }

    Err(format_err!(
        "File {} not found in {}",
        sub_path.to_string_lossy(),
        path_to_zip.to_string_lossy()
    ))
}

fn extract_file_from_tar(archive_path: &PathBuf, file_path: &PathBuf) -> Result<Vec<u8>> {
    extract_file_from_tar_data(&fs::read(archive_path)?, file_path)
}

fn extract_file_from_xz_tar(archive_path: &PathBuf, file_path: &PathBuf) -> Result<Vec<u8>> {
    let file = fs::File::open(archive_path)?;
    let mut xz = XzDecoder::new(file);

    let mut buf = Vec::new();
    xz.read_to_end(&mut buf)?;

    extract_file_from_tar_data(&buf, file_path)
}

fn extract_file_from_tar_data(buf: &Vec<u8>, file_path: &PathBuf) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(buf);

    let mut archive = Archive::new(&mut cursor);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?;

        let entry_path_str = entry_path.to_string_lossy();

        if strip_first_level(&entry_path_str) == format_subpath(file_path).to_string_lossy() {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            return Ok(content);
        }
    }

    Err(format_err!(
        "Path {} not found in tar-file.",
        file_path.to_string_lossy()
    ))
}

fn strip_first_level(s: &str) -> String {
    let mut components = s.split('/').collect::<Vec<_>>();

    if components.len() > 1 {
        components.remove(0);

        let stripped_path = components.join("/");

        return stripped_path;
    } else {
        s.to_string()
    }
}
pub fn format_subpath(subpath: &PathBuf) -> PathBuf {
    match subpath.strip_prefix("/") {
        Ok(p) => p.to_path_buf(),
        Err(_) => subpath.clone(),
    }
}

fn parse_auto_source(auto: &str) -> Result<FileSource> {
    if auto.chars().filter(|&c| c == ':').count() == 1
        && !auto.contains("#")
        && (auto.contains(".tar") || auto.contains(".zip") || auto.contains(".tar.xz"))
    {
        let (a, p) = auto.split_once(":").expect("just checked : count");
        return Ok(FileSource::Archive {
            archive: PathBuf::from(a),
            path: PathBuf::from(p),
        });
    }
    source_from_string_simple(auto)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_parse_auto_sources() {
        assert_eq!(
            parse_auto_source("path/to/archive.tar:path/in/archive").unwrap(),
            FileSource::Archive {
                archive: PathBuf::from("path/to/archive.tar"),
                path: PathBuf::from("path/in/archive")
            }
        );
        assert_eq!(
            parse_auto_source("repo#eaf33129cdee0501af69c04c8d4068c5bf6cbfe1:path").unwrap(),
            FileSource::Git {
                repo: "repo".to_string(),
                id: "eaf33129cdee0501af69c04c8d4068c5bf6cbfe1".to_string(),
                path: PathBuf::from("path")
            }
        );
    }
}
