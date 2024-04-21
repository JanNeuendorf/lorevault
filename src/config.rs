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
    pub fn new(files: Vec<File>, variables: HashMap<String, String>) -> Result<Self> {
        return Self {
            content: files,
            variables,
            variables_set: false,
        }
        .set_variables();
    }
    pub fn get_active(&self, tags: &Vec<String>) -> Result<Vec<File>> {
        if !self.variables_set {
            return Err(format_err!("Variables must have been set to get file list"));
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
        Ok(conf.set_variables()?)
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

    fn set_variables(&self) -> Result<Self> {
        let mut new = self.clone();
        new.content = new.content.set_variables(&self.variables)?;
        Ok(Self {
            variables: new.variables,
            variables_set: true,
            content: new.content,
        })
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
