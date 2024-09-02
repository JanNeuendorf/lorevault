use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum FileSource {
    #[serde(rename = "local")]
    Local { path: PathBuf },
    #[serde(rename = "http")]
    Download { url: String },
    #[serde(rename = "sftp")]
    Sftp {
        user: String,
        service: String,
        path: PathBuf,
        port: Option<usize>,
    },
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
    #[serde(untagged)]
    Auto(String),
}

impl fmt::Display for FileSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileSource::Local { path } => write!(f, "{}", path.display()),
            FileSource::Download { url } => write!(f, "{}", url),
            FileSource::Sftp {
                user,
                service,
                path,
                ..
            } => write!(f, "{}@{}:{}", user, service, path.display()),
            FileSource::Git { repo, id, path } => write!(f, "{}#{}:{}", repo, id, path.display()),
            FileSource::Text { .. } => write!(f, "Custom text"),

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
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.green}{spinner:.green} {msg}")
                        .context("Failed because progress bar")?,
                );
                spinner.set_message(format!("Loading: {}", url));
                spinner.enable_steady_tick(Duration::from_millis(50));
                let response = reqwest::blocking::get(url)?;
                let bytes = response.error_for_status()?.bytes()?.to_vec();
                spinner.finish_with_message(format!("Loaded: {}", url));
                Ok(bytes)
            }
            FileSource::Git {
                repo,
                id: commit,
                path,
            } => get_git_file(commit, path, repo),
            FileSource::Text { content, .. } => Ok(content.clone().into_bytes()),
            FileSource::Sftp {
                user,
                service,
                path,
                port,
            } => get_file_over_sftp(user, service, path, *port),
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

pub fn get_commit_from_string(repo: &Repository, input: &str) -> Result<String> {
    let obj = repo.revparse_single(input.trim()).context(format!(
        "Could not find commit for id: {} revparse failed",
        input
    ))?;
    if let Some(commit) = obj.as_commit() {
        let commit_string = commit.id().to_string();
        info!("ID {} matched to commit {}", input, commit_string);
        return Ok(commit_string);
    }

    Err(format_err!("Could not find commit for id: {}", input))
}

pub fn get_git_repo(repo_path: &str) -> Result<Repository> {
    let repo: Repository;
    if is_url_or_ssh(repo_path) {
        repo = match fetch_repo_from_cache(repo_path) {
            Ok(r) => r,
            Err(_) => clone_repository(repo_path)?,
        };
    } else {
        if PathBuf::from(repo_path).is_relative() {
            return Err(format_err!("Relative paths are not allowed: {}", repo_path));
        }

        repo = Repository::open(repo_path)?;
    }
    Ok(repo)
}
pub fn is_url(path: &str) -> bool {
    path.to_string().starts_with("http://") || path.to_string().starts_with("https://")
}
pub fn is_url_or_ssh(path: &str) -> bool {
    is_url(path) || (path.contains('@') && path.contains(':'))
}

fn cache_name(url: impl AsRef<str>) -> PathBuf {
    PathBuf::from(compute_hash(&url.as_ref().bytes().collect()))
}

fn bare_clone(from: &str, to: &PathBuf) -> Result<Repository> {
    let auth = GitAuthenticator::default();
    let git_config = git2::Config::open_default()?;
    let mut repo_builder = git2::build::RepoBuilder::new();
    let mut fetch_options = git2::FetchOptions::new();
    let mut remote_callbacks = git2::RemoteCallbacks::new();

    remote_callbacks.credentials(auth.credentials(&git_config));
    fetch_options.remote_callbacks(remote_callbacks);
    repo_builder.fetch_options(fetch_options);

    let repo = repo_builder
        .bare(true)
        .remote_create(|repo, name, url| repo.remote_with_fetch(name, url, "+refs/*:refs/*"))
        .clone(from, to)?;
    Ok(repo)
}
fn clone_repository(repo_url: &str) -> Result<Repository> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green}{spinner:.green} {msg}")
            .context("Failed because progress bar")?,
    );
    spinner.set_message(format!("Cloning: {}", repo_url));
    spinner.enable_steady_tick(Duration::from_millis(50));
    let cachedir = match CACHEDIR.get() {
        Some(cd) => cd,
        None => {
            init_cache_dir()?;
            CACHEDIR
                .get()
                .context("Could not establish cache directory for cloned repos.")?
        }
    };

    let repo = bare_clone(
        repo_url,
        &cachedir
            .path()
            .join(cache_name(repo_url))
            .as_path()
            .to_path_buf(),
    )?;
    spinner.finish_with_message(format!("Cloned: {}", repo_url));

    Ok(repo)
}

pub fn init_cache_dir() -> Result<PathBuf> {
    let tmpdir = TempDir::new()?;
    let path = tmpdir.path().to_path_buf();
    let result = CACHEDIR.set(tmpdir);
    info!("Cache directory: {:?}", path);
    match result {
        Ok(_) => Ok(path),
        Err(td) => Err(format_err!("Could not init cachedir {:?}", td)),
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

fn get_file_over_sftp(
    user: &str,
    service: &str,
    path: &PathBuf,
    port: Option<usize>,
) -> Result<Vec<u8>> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green}{spinner:.green} {msg}")
            .context("Failed because progress bar")?,
    );
    spinner.set_message(format!("loading: {}@{}:{}", user, service, path.display()));
    spinner.enable_steady_tick(Duration::from_millis(50));

    let port = port.unwrap_or(22);
    let tcp = TcpStream::connect(format!("{}:{}", service, port))?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_agent(user)?;
    let sftp = sess.sftp()?;
    let mut remote_file = sftp.open(path)?;
    let mut contents = Vec::new();
    remote_file.read_to_end(&mut contents)?;
    spinner.finish_with_message(format!("loaded: {}@{}:{}", user, service, path.display()));
    Ok(contents)
}

pub fn format_subpath(subpath: &PathBuf) -> PathBuf {
    match subpath.strip_prefix("/") {
        Ok(p) => p.to_path_buf(),
        Err(_) => subpath.clone(),
    }
}

fn parse_sftp(sftp_url: &str) -> Result<(String, String, String)> {
    let parts: Vec<&str> = sftp_url.split('@').collect();
    if parts.len() != 2 {
        return Err(format_err!("invalid ssh string"));
    }
    let user = parts[0].to_string();

    let service_and_path: Vec<&str> = parts[1].splitn(2, ':').collect();
    if service_and_path.len() != 2 {
        return Err(format_err!("invalid ssh string"));
    }
    let service = service_and_path[0].to_string();
    let path = service_and_path[1].to_string();

    Ok((user, service, path))
}

fn parse_auto_source(auto: &str) -> Result<FileSource> {
    if !is_repo(auto) && !is_url(auto) && auto.contains("@") && auto.contains(":") {
        let (user, service, path) = parse_sftp(auto)?;
        return Ok(FileSource::Sftp {
            user,
            service,
            path: PathBuf::from(path),
            port: None,
        });
    }
    if is_url(auto) && !is_repo(auto) {
        return Ok(FileSource::Download {
            url: auto.to_string(),
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
            parse_auto_source("repo#eaf33129cdee0501af69c04c8d4068c5bf6cbfe1:path").unwrap(),
            FileSource::Git {
                repo: "repo".to_string(),
                id: "eaf33129cdee0501af69c04c8d4068c5bf6cbfe1".to_string(),
                path: PathBuf::from("path")
            }
        );
        assert_eq!(
            parse_auto_source("username@service.com:some/path").unwrap(),
            FileSource::Sftp {
                user: "username".to_string(),
                service: "service.com".to_string(),
                path: PathBuf::from("some/path"),
                port: None
            }
        );
    }
}
