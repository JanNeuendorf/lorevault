use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde_as]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    #[serde(skip)]
    variables_set: bool, // This is just a flag to ensure that we do not work with a config before tha variables have been replaced.
    #[serde(default, alias = "var")] // The alias lets us write var.key=value in the toml file.
    variables: HashMap<String, String>,
    #[serde(rename = "file", default)]
    content: Vec<File>,
    #[serde(default)]
    #[serde(rename = "include")]
    inclusions: Vec<Inclusion>,
    #[serde(default)]
    #[serde(rename = "directory")]
    directories: Vec<Directory>,
    #[serde(rename = "default", default)]
    pub default_tags: Vec<String>,
}

impl Config {
    // This gets all files that should be included given the list of tags.
    // It should error if two tagged files or two untagged files have the same path.
    // If an untagged file and a tagged file have the same path, only the tagged one is active.
    pub fn get_active(&self, given_tags: &Vec<String>) -> Result<Vec<File>> {
        if !self.variables_set {
            return Err(format_err!("Variables must have been set to get file list"));
        }
        let defined_tags = self.tags();

        let given_tags = given_tags.iter().map(|t| t.trim()).collect::<Vec<_>>();
        let positive_tags = given_tags
            .iter()
            .filter(|t| !t.starts_with("!"))
            .map(|t| t.to_string())
            .collect::<Vec<_>>();

        let negative_tags = given_tags
            .iter()
            .filter(|t| t.starts_with("!"))
            .map(|t| t.strip_prefix("!").expect("could not remove !").to_string())
            .collect::<Vec<_>>();

        for nt in &negative_tags {
            if positive_tags.contains(&nt) {
                return Err(format_err!("You try to negate a tag while activating it"));
            }
        }

        let tags = &vecset(vec![self.default_tags.clone(), positive_tags])
            .iter()
            .filter(|p| !negative_tags.contains(p))
            .map(|t| t.to_string())
            .collect::<Vec<_>>();

        for requested_tag in tags {
            if !defined_tags.contains(requested_tag) {
                return Err(format_err!(
                    "The tag {} is not defined in the config file.",
                    requested_tag
                ));
            }
        }
        let mut new_content = vec![];
        let mut file_list = self.content.clone();
        for inc in &self.inclusions {
            file_list.append(&mut inc.get_files()?)
        }
        for dir in &self.directories {
            file_list.append(&mut dir.get_active(&tags)?)
        }
        let mut paths = vec![];
        let tagged_paths = file_list
            .iter()
            .filter(|i| i.get_tags().iter().any(|ct| tags.contains(ct)))
            .map(|i| i.get_path().to_owned())
            .collect::<Vec<PathBuf>>();
        for item in &file_list {
            if !item.is_active(tags) {
                continue;
            }
            if item.get_tags().is_empty() && tagged_paths.contains(&item.get_path()) {
                continue;
            }
            if paths.contains(&item.get_path()) {
                return Err(format_err!(
                    "There are two files for path {}",
                    &item.get_path().to_string_lossy()
                ));
            }

            new_content.push(item.clone());
            paths.push(item.get_path().clone())
        }

        Ok(new_content)
    }

    fn from_filesource(source: &FileSource, allow_local: bool, hash: Option<&str>) -> Result<Self> {
        let data = match source {
            FileSource::Local { path } => {
                if path.is_relative() && !allow_local {
                    return Err(format_err!(
                        "Trying to load config from relative path {:?}",
                        path
                    ));
                }

                fs::read(path).context(format!("Could not load config {}", path.display()))?
            }
            FileSource::Git { .. } => source.fetch()?,
            _ => {
                return Err(format_err!("Loading config from unsupported filesource."));
            }
        };
        // This is only relevant if the config was included.
        if let Some(hash) = hash {
            if compute_hash(&data) != hash {
                return Err(format_err!("Hash of loaded config did not match."));
            }
        }
        let toml_string = String::from_utf8(data)?;

        let conf: Self = toml::from_str(&toml_string)?;

        Ok(conf.set_variables(source)?)
    }

    // The allow_local flag is to make sure that local files are only valid, when the path was passed on the cli.
    pub fn from_general_path(
        general_path: &str,
        allow_local: bool,
        hash: Option<&str>,
    ) -> Result<Self> {
        let source = cli::source_from_string_simple(general_path)?;
        Self::from_filesource(&source, allow_local, hash)
    }
    #[allow(unused)] // This is handy if one wants to see what a new field looks like in a .toml file.
    pub fn write(&self, path: &PathBuf) -> Result<()> {
        let toml = toml::to_string_pretty(self)?;
        fs::write(path, toml)?;
        Ok(())
    }

    pub fn set_variables(&self, source: &FileSource) -> Result<Self> {
        if self.variables_set {
            // This should never happen.
            return Err(format_err!(
                "Trying to set variables twice for the same config."
            ));
        }
        let mut new = self.clone();
        if self
            .variables
            .keys()
            .into_iter()
            .any(|k| k.starts_with("SELF_") || k.starts_with("#") || k.starts_with("!"))
        {
            return Err(format_err!(
                "Variables starting with SELF_,! or # are protected."
            ));
        }

        let mut vars = self.variables.clone();
        match source {
            FileSource::Git { repo, id, path } => {
                vars.insert("SELF_ID".to_string(), id.to_string());
                vars.insert("SELF_REPO".to_string(), repo.to_string());
                vars.insert(
                    "SELF_NAME".to_string(),
                    path.file_name()
                        .context("Config must have a name.")?
                        .to_str()
                        .context("Path must be printable")?
                        .to_string(),
                );
                let repostring = if is_url_or_ssh(&repo) {
                    repo.to_string()
                } else {
                    let repopath = PathBuf::from(repo).canonicalize()?;
                    repopath
                        .to_str()
                        .context("Could not print repo path")?
                        .to_string()
                };

                vars.insert("SELF_ROOT".to_string(), format!("{}#{}:", repostring, id));
            }
            FileSource::Local { path } => {
                let parent = path
                    .canonicalize()
                    .context("Path to local config could not be converted to absolute path.")?
                    .parent()
                    .context("A local config must have a parent dir.")?
                    .to_str()
                    .context("Could not parse the config path to string.")?
                    .to_string();
                vars.insert("SELF_PARENT".to_string(), parent.clone());
                vars.insert("SELF_ROOT".to_string(), parent);
                vars.insert(
                    "SELF_NAME".to_string(),
                    path.file_name()
                        .context("Config must have a name.")?
                        .to_str()
                        .context("Path must be printable")?
                        .to_string(),
                );
            }
            _ => {
                // This should be unreachable.
                return Err(format_err!(
                    "Configs should only be read from repos or local paths."
                ));
            }
        }
        vars = resolve_variable_inter_refs(&vars)?;

        new.content = new.content.set_variables(&vars)?;
        new.directories = new.directories.set_variables(&vars)?;
        new.inclusions = new.inclusions.set_variables(&vars)?;
        let conf = Self {
            variables: new.variables,
            variables_set: true,
            content: new.content,
            inclusions: new.inclusions,
            directories: new.directories,
            default_tags: self.default_tags.clone(),
        };
        // This is a little ugly and the validation might be missed.
        validate_tags(&conf.tags())?;
        Ok(conf)
    }

    pub fn tags(&self) -> Vec<String> {
        let mut taglists = vec![];
        for file in &self.content {
            taglists.push(file.tags.clone().unwrap_or(vec![]));
            for e in &file.edits {
                taglists.push(e.get_tags().clone())
            }
        }
        for inc in &self.inclusions {
            taglists.push(inc.tags.clone().unwrap_or(vec![]));
        }
        for d in &self.directories {
            taglists.push(d.get_tags());
        }

        vecset(taglists)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub path: PathBuf,
    pub tags: Option<Vec<String>>,
    pub hash: Option<String>,
    #[serde(rename = "sources", alias = "source")]
    pub sources: Vec<FileSource>,
    #[serde(rename = "edit", default)]
    pub edits: Vec<FileEdit>,
}

impl File {
    pub fn get_tags(&self) -> Vec<String> {
        if let Some(t) = &self.tags {
            t.clone()
        } else {
            vec![]
        }
    }
    pub fn get_path(&self) -> PathBuf {
        format_subpath(&self.path)
    }
    fn is_active(&self, reqtags: &Vec<String>) -> bool {
        let tags = self.get_tags();
        if tags.len() == 0 {
            return true;
        }
        for t in reqtags {
            if tags.contains(t) {
                return true;
            }
        }
        false
    }
    pub fn from_reference_unchecked(&self, data: &Vec<u8>, tags: &Vec<String>) -> Result<Vec<u8>> {
        if self.edits.len() == 0 {
            return Ok(data.clone());
        } else {
            let mut strdata = String::from_utf8(data.clone())?;
            for edit in &self.edits {
                if !edit.is_active(tags) {
                    continue;
                }
                strdata = edit.apply(&strdata)?;
            }
            return Ok(strdata.into_bytes());
        }
    }
    pub fn build(&self, tags: &Vec<String>) -> Result<Vec<u8>> {
        let data = fetch_first_valid(&self.sources, &self.hash)?;
        self.from_reference_unchecked(&data, tags)
    }
}

fn fetch_first_valid(sources: &Vec<FileSource>, hash: &Option<String>) -> Result<Vec<u8>> {
    for s in sources {
        let result = s.fetch();

        if result.is_ok() {
            if hash.is_none() {
                return result;
            } else {
                if hash.as_ref().expect("must be some")
                    == &compute_hash(&result.as_ref().expect("ref must exist"))
                {
                    return result;
                } else {
                    red(format!("Invalid hash {}", &s)); // This might not kill the program, but it is bad enough to warrant red text.
                }
            }
        } else {
            yellow(format!(
                "Invalid source {} \nError: {}",
                &s,
                result.err().expect("error branch")
            ));
        }
    }
    return Err(format_err!("No valid source in list."));
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Inclusion {
    pub config: String,
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub with_tags: Vec<String>,
    #[serde(default, rename = "path")]
    pub subfolder: PathBuf,
    pub hash: Option<String>,
}
impl Inclusion {
    pub fn get_files(&self) -> Result<Vec<File>> {
        let config =
            Config::from_general_path(&self.config, false, self.hash.as_ref().map(|s| s.as_str()))?;
        let mut files: Vec<File> = vec![];
        for original_file in config.get_active(&self.with_tags)? {
            files.push(File {
                path: self.subfolder.join(format_subpath(&original_file.path)),
                tags: self.tags.clone(),
                hash: original_file.hash,
                sources: original_file.sources,
                edits: include_edits(&original_file.edits, &self.tags.clone().unwrap_or(vec![])),
            })
        }
        for d in &config.directories {
            files.append(&mut d.get_active(&self.with_tags)?);
        }
        // Including an empty file is forbidden, because lorevault knows only files and no empty directories.
        if files.len() == 0 {
            return Err(format_err!(
                "Including zero files from a different config is not allowed. ({})",
                self.config
            ));
        }

        Ok(files)
    }
}

// We don't want tags to start with a ! or be a variant of the word default.

fn validate_tags(tags: &Vec<String>) -> Result<()> {
    for t in tags {
        if t.starts_with("!") {
            return Err(format_err!(
                "Tag names can not start with an exclamation mark."
            ));
        }
        if t.trim().to_lowercase() == "default".to_string() {
            return Err(format_err!("A tag can not be named \"default\""));
        }
    }
    Ok(())
}
