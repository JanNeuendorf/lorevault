use crate::*;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde_as]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    #[serde(skip)]
    variables_set: bool,
    #[serde(default, alias = "var")]
    variables: HashMap<String, String>,
    #[serde(rename = "file", alias = "files")]
    content: Vec<File>,
}

impl Config {
    #[allow(unused)]
    pub fn new(files: Vec<File>, variables: HashMap<String, String>) -> Self {
        return Self {
            content: files,
            variables,
            variables_set: false,
        };
    }
    pub fn get_active(&self, tags: &Vec<String>) -> Result<Vec<File>> {
        if !self.variables_set {
            return Err(format_err!("Variables must have been set to get file list"));
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
        Ok(new_content)
    }
    pub fn get_all(&self) -> Vec<File> {
        self.content.clone()
    }

    fn from_filesource(source: &FileSource) -> Result<Self> {
        let data = match source {
            FileSource::Local { path } => fs::read(path)?,
            _ => source.fetch()?,
        };
        let toml_string = String::from_utf8(data)?;
        let conf: Self = toml::from_str(&toml_string)?;
        Ok(conf.set_variables(source)?)
    }

    pub fn from_general_path(general_path: &str) -> Result<Self> {
        let source = cli::source_from_string(general_path)?;
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
            FileSource::Git { repo, commit, .. } => {
                vars.insert("SELF_COMMIT".to_string(), commit.to_string());
                vars.insert("SELF_REPO".to_string(), repo.to_string());
            }
            FileSource::Local { path } => {
                vars.insert(
                    "SELF_PARENT".to_string(),
                    path.parent()
                        .context("A local config must have a parent dir.")?
                        .to_str()
                        .context("Could not parse the config path to string.")?
                        .to_string(),
                );
            }
            _ => {}
        }

        new.content = new.content.set_variables(&vars)?;
        Ok(Self {
            variables: new.variables,
            variables_set: true,
            content: new.content,
        })
    }
    pub fn tags(&self) -> Vec<String> {
        let mut taglists = vec![];
        for file in &self.content {
            taglists.push(file.tags.clone().unwrap_or(vec![]));
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
    #[serde(rename = "source")]
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
