use crate::cli::source_from_string;
use crate::sources::FileSource;
use anyhow:: Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use anyhow::format_err;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "file")]
    content: Vec<File>,
}

impl Config {
    pub fn get_active(&self, tags: &Vec<String>) -> Result<Vec<File>> {
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
                return Err(format_err!("There are two files for path {}",&item.get_path().to_string_lossy()));
            }
            new_content.push(item.clone());
            paths.push(item.get_path().clone())
        }
        Ok(new_content)
    }
    pub fn get_all(&self) -> Vec<File> {
        self.content.clone()
    }


    fn from_filesource(source:&FileSource)->Result<Self>{
        let data=match source{
            FileSource::Local { path }=>{
                fs::read(path)?
            },
            _=>{source.fetch()?}
        };
        let toml_string=String::from_utf8(data)?;
        Ok(toml::from_str(&toml_string)?)
    }

    pub fn from_general_path(general_path:&str)->Result<Self>{
        let source=source_from_string(general_path)?;
        Self::from_filesource(&source)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]

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
