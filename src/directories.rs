use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Directory {
    count: Option<usize>,
    path: PathBuf,
    tags: Option<Vec<String>>,
    #[serde(rename = "sources", alias = "source")]
    sources: Vec<DirSource>,
    #[serde(default)]
    ignore_hidden: bool,
}

impl Directory {
    pub fn get_tags(&self) -> Vec<String> {
        self.tags.clone().unwrap_or(vec![])
    }

    fn is_active(&self, tags: &Vec<String>) -> bool {
        if self.get_tags().len() == 0 {
            return true;
        }
        for requested in self.get_tags() {
            if tags.contains(&requested) {
                return true;
            }
        }
        return false;
    }
    pub fn get_active(&self, tags: &Vec<String>) -> Result<Vec<File>> {
        if self.is_active(tags) {
            self.get_all_files()
        } else {
            Ok(vec![])
        }
    }

    pub fn get_all_files(&self) -> Result<Vec<File>> {
        let anyhow::Result::Ok((source, list)) = list_first_valid(&self.sources) else {
            return Err(format_err!(
                "No valid source for directory: {}",
                self.path.display()
            ));
        };

        if let Some(c) = self.count {
            if c != list.len() {
                return Err(format_err!(
                    "Expected {} files for directory {}, found {}",
                    c,
                    &self.path.display(),
                    list.len()
                ));
            }
        }
        let mut files: Vec<File> = vec![];
        for subpath in list {
            if self.ignore_hidden && subpath.display().to_string().starts_with(".") {
                continue;
            }
            files.push(File {
                path: self.path.clone().join(&subpath),
                tags: self.tags.clone(),
                hash: None,
                sources: vec![source.get_single_file_source(&subpath)?],
                edits: vec![],
            })
        }
        if files.len() == 0 {
            return Err(format_err!(
                "No files found for directory {}",
                self.path.display()
            ));
        }
        Ok(files)
    }
}

fn list_first_valid(ds: &Vec<DirSource>) -> Result<(&DirSource, Vec<PathBuf>)> {
    for s in ds {
        if let anyhow::Result::Ok(l) = s.list() {
            return Ok((s, l));
        }
        match s.list() {
            Ok(l) => return Ok((s, l)),
            Err(msg) => yellow(format!("Invalid directory source {} \nError: {}", &s, msg)),
        }
    }
    Err(format_err!("No valid source for directory"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum DirSource {
    #[serde(rename = "local")]
    Local { path: PathBuf },
    #[serde(rename = "git")]
    Git {
        repo: String,
        id: String,
        path: PathBuf,
    },
    #[serde(untagged)]
    Auto(String),
}

impl fmt::Display for DirSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local { path } => write!(f, "{}", path.display()),
            Self::Auto(a) => write!(f, "{}", a),
            Self::Git { repo, id, path } => write!(f, "{}#{}:{}", repo, id, path.display()),
        }
    }
}
impl DirSource {
    pub fn list(&self) -> Result<Vec<PathBuf>> {
        let list = match self {
            DirSource::Git { repo, id, path } => {
                if !is_url_or_ssh(&repo) & PathBuf::from(repo).is_relative() {
                    return Err(format_err!("Path to repo must be absolute {}", repo));
                }
                let repo = get_git_repo(&repo)?;

                list_files_in_repo(&repo, id, path)?
            }
            DirSource::Local { path } => {
                if path.is_relative() {
                    return Err(format_err!(
                        "Path to directory must be absolute {}",
                        path.display()
                    ));
                }
                list_files_in_folder(path)?
            }
            DirSource::Auto(auto) => {
                let parsed = parse_auto_dir_source(auto)?;
                parsed.list()?
            }
        };
        Ok(list.iter().map(|p| format_subpath(p)).collect())
    }
    fn get_single_file_source(&self, subpath: &PathBuf) -> Result<FileSource> {
        let subpath = format_subpath(subpath);
        match self {
            DirSource::Git { repo, id, path } => Ok(FileSource::Git {
                repo: repo.to_string(),
                id: id.to_string(),
                path: path.join(subpath),
            }),
            DirSource::Local { path } => Ok(FileSource::Local {
                path: path.join(subpath),
            }),
            DirSource::Auto(auto) => {
                let parsed = parse_auto_dir_source(auto)?;
                parsed.get_single_file_source(&subpath)
            }
        }
    }
}

fn list_files_in_repo(repo: &Repository, id: &str, folder_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let folder_path = match folder_path.strip_prefix("/") {
        Ok(s) => s,
        _ => folder_path,
    }
    .to_owned();
    let mut full_paths = full_paths_in_repo(repo, id, &folder_path)?;
    let to_remove = format_subpath(&folder_path);
    for p in &mut full_paths {
        *p = p.strip_prefix(&to_remove)?.to_path_buf();
    }
    Ok(full_paths)
}

fn full_paths_in_repo(repo: &Repository, id: &str, folder_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let commit_string = get_commit_from_string(repo, id)?;
    let commit = repo.find_commit(Oid::from_str(&commit_string)?)?;
    let mut paths = Vec::new();
    let tree = commit.tree()?;
    let entry =
        if &folder_path.display().to_string() == "" || &folder_path.display().to_string() == "/" {
            tree
        } else {
            let std::result::Result::Ok(entry) = tree
                .get_path(&std::path::Path::new(&format_subpath(folder_path)))?
                .to_object(repo)?
                .into_tree()
            else {
                return Err(format_err!("Entry is not a tree"));
            };
            entry
        };

    for entry in entry.iter() {
        if entry.kind() == Some(git2::ObjectType::Tree) {
            let subfolder_path = format!(
                "{}/{}",
                folder_path.display(),
                entry.name().context("Failed to get entry name")?
            );
            paths.extend(full_paths_in_repo(
                repo,
                id,
                &format_subpath(&PathBuf::from(subfolder_path)),
            )?);
        } else if entry.kind() == Some(git2::ObjectType::Blob) {
            let full_path = format!(
                "{}/{}",
                folder_path.display(),
                entry.name().context("Failed to get entry name")?
            );
            paths.push(PathBuf::from(full_path));
        }
    }

    Ok(paths)
}

fn list_files_in_folder(folder_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let full_paths = get_full_paths_in_folder(folder_path)?;
    let mut trimmed = vec![];
    for p in &full_paths {
        let t = p
            .strip_prefix(folder_path)
            .context("Could not strip prefix from path")?;
        trimmed.push(format_subpath(&t.to_path_buf()));
    }
    Ok(trimmed)
}
fn get_full_paths_in_folder(folder_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_file() {
            let file_path = entry.path();
            files.push(file_path);
        } else if file_type.is_dir() {
            let dir_files = get_full_paths_in_folder(&entry.path())?;
            if dir_files.is_empty() {
                return Err(Error::msg("Empty folders not supported."));
            }
            files.extend(dir_files);
        } else {
            return Err(Error::msg("Only regular files are supported."));
        }
    }

    Ok(files)
}

fn parse_auto_dir_source(auto: &str) -> Result<DirSource> {
    if is_repo(auto) {
        match extract_components(auto) {
            Some((repo, id, path)) => Ok(DirSource::Git {
                repo: repo.into(),
                id: id.into(),
                path: path.into(),
            }),
            None => Err(format_err!(format!("Could not parse repo string {}", auto))),
        }
    } else {
        Ok(DirSource::Local { path: auto.into() })
    }
}

impl VariableCompletion for Directory {
    fn required_variables(&self) -> Result<Vec<String>> {
        Ok(vecset(vec![
            self.sources.required_variables()?,
            self.path.required_variables()?,
        ]))
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        *self = Directory {
            path: self.path.set_single_variable(key, value)?,
            sources: self.sources.set_single_variable(key, value)?,
            ..self.clone()
        };
        return Ok(self.to_owned());
    }
}

impl VariableCompletion for DirSource {
    fn required_variables(&self) -> Result<Vec<String>> {
        match self {
            DirSource::Auto(auto) => auto.required_variables(),
            DirSource::Git { repo, id, path } => Ok(vecset(vec![
                path.required_variables()?,
                repo.required_variables()?,
                id.required_variables()?,
            ])),
            DirSource::Local { path } => path.required_variables(),
        }
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        *self = match self {
            DirSource::Auto(a) => DirSource::Auto(a.set_single_variable(key, value)?),
            DirSource::Git { repo, id, path } => DirSource::Git {
                repo: repo.set_single_variable(key, value)?,
                id: id.set_single_variable(key, value)?,
                path: path.set_single_variable(key, value)?,
            },
            DirSource::Local { path } => DirSource::Local {
                path: path.set_single_variable(key, value)?,
            },
        };
        Ok(self.clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn print_list() {
        let list = list_files_in_folder(&PathBuf::from("testing/testfolder")).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&PathBuf::from("file1.txt")));
        assert!(list.contains(&PathBuf::from("subfolder/file2.txt")));
    }
}
