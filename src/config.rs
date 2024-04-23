use self::{cli::source_from_string_simple, sources::is_url};
use crate::*;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
const INCLUSION_RECURSION_LIMIT: usize = 10;

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
    pub fn get_active(&self, tags: &Vec<String>) -> Result<Vec<File>> {
        if !self.variables_set {
            return Err(format_err!(
                "Variables and inclusions must have been set to get file list"
            ));
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
        for inc in &self.inclusions {
            let files_to_include = inc.get_files()?;
            for f2i in files_to_include {
                if f2i.is_active(tags) {
                    if paths.contains(&f2i.path) {
                        //println!("inc: {:?} file: {:?}",&inc,&f2i);
                        return Err(format_err!(
                            "There are two files for path {}",
                            &f2i.get_path().to_string_lossy()
                        ));
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

    fn from_filesource(source: &FileSource) -> Result<Self> {
        let data = match source {
            FileSource::Local { path } => fs::read(path)
                .context(format!("Could not load config {}", path.to_string_lossy()))?,
            _ => source.fetch()?,
        };
        let toml_string = String::from_utf8(data)?;
        let conf: Self = toml::from_str(&toml_string)?;
        Ok(conf.set_variables(source)?)
    }

    pub fn from_general_path(general_path: &str) -> Result<Self> {
        let source = cli::source_from_string_simple(general_path)?;
        Self::from_filesource(&source)
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
            FileSource::Git { repo, commit, path } => {
                vars.insert("SELF_COMMIT".to_string(), commit.to_string());
                vars.insert("SELF_REPO".to_string(), repo.to_string());
                vars.insert(
                    "SELF_NAME".to_string(),
                    path.file_name()
                        .context("Config must have a name.")?
                        .to_str()
                        .context("Path must be printable")?
                        .to_string(),
                );
                let repostring = if is_url(&repo) {
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
                vars.insert(
                    "SELF_PARENT".to_string(),
                    path.parent()
                        .context("A local config must have a parent dir.")?
                        .canonicalize()?
                        .to_str()
                        .context("Could not parse the config path to string.")?
                        .to_string(),
                );
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
        }
        for inc in &self.inclusions {
            taglists.push(inc.tags.clone().unwrap_or(vec![]));
        }
        vecset(taglists)
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Inclusion {
    pub config: String,
    #[serde(alias = "required_tags")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub with_tags: Vec<String>,
    #[serde(default, alias = "path")]
    pub subfolder: PathBuf,
}
impl Inclusion {
    pub fn get_files(&self) -> Result<Vec<File>> {
        let config_source = source_from_string_simple(&self.config)?;
        let config = Config::from_filesource(&config_source)?;
        let mut files: Vec<File> = vec![];
        for original_file in config.get_active(&self.with_tags)? {
            files.push(File {
                path: self.subfolder.join(format_subpath(&original_file.path)),
                tags: self.tags.clone(),
                hash: original_file.hash,
                sources: original_file.sources,
            })
        }

        Ok(files)
    }
}

fn get_next_inclusion_level(cfgs: &Vec<String>) -> Result<Vec<String>> {
    let mut tmp = vec![];

    for cfg in cfgs {
        tmp.push(
            Config::from_general_path(cfg)?
                .inclusions
                .iter()
                .map(|inc| inc.config.to_string())
                .collect::<Vec<String>>(),
        );
    }
    Ok(vecset(tmp))
}

pub fn check_recursion(cfg: &str) -> Result<()> {
    let mut next_deps = vec![cfg.to_string()];
    for _ in 0..INCLUSION_RECURSION_LIMIT {
        next_deps = get_next_inclusion_level(&next_deps)?;
        if next_deps.len() == 0 {
            return Ok(());
        }
    }

    Err(format_err!(
        "The inclusions are too deep (max depth={}) or recursive.",
        INCLUSION_RECURSION_LIMIT
    ))
}
