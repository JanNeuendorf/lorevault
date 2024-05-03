use crate::*;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
const INCLUSION_RECURSION_LIMIT: usize = 10; // The depth of inclusions of other config files.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde_as]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    #[serde(skip)]
    variables_set: bool,
    #[serde(default, alias = "var")]
    variables: HashMap<String, String>,
    #[serde(rename = "file", alias = "files", default)]
    content: Vec<File>,
    #[serde(default)]
    #[serde(alias = "include")]
    inclusions: Vec<Inclusion>,
}

impl Config {
    // This gets all files that should be included given the list of tags.
    // It should error if two tagged files or two untagged files have the same path.
    // If an untagged file and a tagged file have the same path, only the tagged one is active.
    pub fn get_active(&self, tags: &Vec<String>) -> Result<Vec<File>> {
        if !self.variables_set {
            return Err(format_err!("Variables must have been set to get file list"));
            //Should never happen, as long as we call set_variables() first.
        }
        let defined_tags = self.tags();
        for requested_tag in tags {
            if !defined_tags.contains(requested_tag) {
                return Err(format_err!(
                    "The tag {} is not defined in the config file.",
                    requested_tag
                ));
            }
        }
        let mut new_content = vec![];
        let mut paths = vec![];
        let tagged_paths = self
            .content
            .iter()
            .filter(|i| i.get_tags().iter().any(|ct| tags.contains(ct)))
            .map(|i| i.get_path().to_owned())
            .collect::<Vec<PathBuf>>();
        for item in &self.content {
            if !item.is_active(tags) {
                continue;
            }
            if item.get_tags().is_empty() && tagged_paths.contains(item.get_path()) {
                continue;
            }
            if paths.contains(item.get_path()) {
                return Err(format_err!(
                    "There are two files for path {}",
                    &item.get_path().to_string_lossy()
                ));
            }
            new_content.push(item.clone());
            paths.push(item.get_path().clone())
        }
        // The logic for included files is different in a subtle way.
        // Even tagged included files do not overwrite other untagged files. They give an error, if the paths collide.
        // This prevents a situation where an update to the included config overwrites the content of files defined locally.
        for inc in &self.inclusions {
            let files_to_include = inc.get_files()?;
            for f2i in files_to_include {
                if f2i.is_active(tags) {
                    if paths.contains(&f2i.path) {
                        if f2i.get_tags().len() == 0 && tagged_paths.contains(f2i.get_path()) {
                            continue;
                        } else {
                            return Err(format_err!(
                                "There are two files for path {}",
                                &f2i.get_path().to_string_lossy()
                            ));
                        }
                    }
                    paths.push(f2i.path.clone());
                    new_content.push(f2i);
                }
            }
        }

        Ok(new_content)
    }
    pub fn get_all(&self) -> Result<Vec<File>> {
        let mut new_content = self.content.clone();
        for inc in &self.inclusions {
            let files_to_include = inc.get_files()?;
            for f2i in files_to_include {
                new_content.push(f2i);
            }
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
                info!("Loading config from local file {}", path.display());
                fs::read(path).context(format!("Could not load config {}", path.display()))?
            }
            FileSource::Git { .. } => source.fetch()?,
            _ => {
                return Err(format_err!("Loading config from unsupported filesource."));
            }
        };

        if let Some(hash) = hash {
            if compute_hash(&data) != hash {
                return Err(format_err!("Hash of loaded config did not match."));
            }
        }
        let toml_string = String::from_utf8(data)?;
        if toml_string == include_str!("lorevault_example.toml") {
            return Err(format_err!(
                "The example must be modified to make it functional."
            ));
        }

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
        info!("Loading config from source {:?}", source);
        Self::from_filesource(&source, allow_local, hash)
    }
    #[allow(unused)]
    pub fn write(&self, path: &PathBuf) -> Result<()> {
        let toml = toml::to_string_pretty(self)?;
        fs::write(path, toml)?;
        Ok(())
    }

    pub fn set_variables(&self, source: &FileSource) -> Result<Self> {
        let mut new = self.clone();
        if self
            .variables
            .keys()
            .into_iter()
            .any(|k| k.starts_with("SELF_"))
        {
            return Err(format_err!("Variables starting with SELF_ are protected."));
        }

        let mut vars = self.variables.clone();
        match source {
            FileSource::Git {
                repo,
                id: commit,
                path,
            } => {
                vars.insert("SELF_ID".to_string(), commit.to_string());
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

                vars.insert(
                    "SELF_ROOT".to_string(),
                    format!("{}#{}:", repostring, commit),
                );
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
                vars.insert("SELF_PARENT".to_string(), parent);
                vars.insert(
                    "SELF_ROOT".to_string(),
                    vars.get("SELF_PARENT").expect("just set").clone(),
                );
                vars.insert(
                    "SELF_NAME".to_string(),
                    path.file_name()
                        .context("Config must have a name.")?
                        .to_str()
                        .context("Path must be printable")?
                        .to_string(),
                );
            }
            _ => {}
        }
        vars = resolve_variable_inter_refs(&vars)?;
        info!("Variables in {:?}: {:?} ", source, vars);
        new.content = new.content.set_variables(&vars)?;
        new.inclusions = new.inclusions.set_variables(&vars)?;
        Ok(Self {
            variables: new.variables,
            variables_set: true,
            content: new.content,
            inclusions: new.inclusions,
        })
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
        vecset(taglists)
    }
    pub fn is_fully_hardened(&self) -> Result<bool> {
        for f in self.get_all()? {
            if f.hash.is_none() {
                return Ok(false);
            }
        }
        for i in &self.inclusions {
            if !i.is_fully_hardened()? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub path: PathBuf,
    #[serde(alias = "required_tags")]
    pub tags: Option<Vec<String>>,
    pub hash: Option<String>,
    #[serde(alias = "source")]
    pub sources: Vec<FileSource>,
    #[serde(alias = "edit", default)]
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
    pub fn get_path(&self) -> &PathBuf {
        &self.path
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
                    red(format!("Invalid hash {:?}", &s));
                }
            }
        } else {
            red(format!(
                "Invalid source {:?} \nError: {}",
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
    #[serde(alias = "required_tags")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub with_tags: Vec<String>,
    #[serde(default, alias = "path", alias = "subdir", alias = "subdirectory")]
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

        Ok(files)
    }
    pub fn is_fully_hardened(&self) -> Result<bool> {
        let config =
            Config::from_general_path(&self.config, false, self.hash.as_ref().map(|s| s.as_str()))?;
        Ok(self.hash.is_some() && config.is_fully_hardened()?)
    }
}

// This is just a helper function to check if the inclusions might be recursive
fn get_next_inclusion_level(cfgs: &Vec<String>) -> Result<Vec<String>> {
    let mut tmp = vec![];

    for cfg in cfgs {
        let allow_local = tmp.len() == 0;
        tmp.push(
            Config::from_general_path(cfg, allow_local, None)?
                .inclusions
                .iter()
                .map(|inc| inc.config.to_string())
                .collect::<Vec<String>>(),
        );
    }
    Ok(vecset(tmp))
}

// This can be used to check for recursion in the inclusions.
// It must be manually called and checked before loading the config file.
pub fn check_recursion(cfg: &str) -> Result<()> {
    let mut next_deps = vec![cfg.to_string()];
    for i in 0..INCLUSION_RECURSION_LIMIT {
        info!("Looking for recursions {} levels deep", i);
        next_deps = get_next_inclusion_level(&next_deps)?;
        info!("Found {} dependencies.", next_deps.len());
        if next_deps.len() == 0 {
            return Ok(());
        }
    }

    Err(format_err!(
        "The inclusions are too deep (max depth={}) or recursive.",
        INCLUSION_RECURSION_LIMIT
    ))
}
