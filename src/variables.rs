use crate::*;
use regex::Regex;
use std::collections::HashSet;

pub trait VariableCompletion: Sized + Clone {
    fn required_variables(&self) -> Result<Vec<String>>;
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self>;
    fn set_variables(&self, map: &HashMap<String, String>) -> Result<Self> {
        let requested = self.required_variables()?;
        let mut new = self.clone();
        for key in &requested {
            let value = map.get(key).context("Required key not in variables")?;
            new.set_single_variable(key, value)?;
        }
        Ok(new)
    }
}

impl VariableCompletion for String {
    fn required_variables(&self) -> Result<Vec<String>> {
        let re = Regex::new(r"\{\{([^{}]+)\}\}").unwrap();
        let mut variables = Vec::new();

        for capture in re.captures_iter(self) {
            if let Some(variable) = capture.get(1) {
                variables.push(variable.as_str().to_owned());
            }
        }
        Ok(variables)
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<String> {
        let new = self.replace(&format!("{{{{{}}}}}", key), value).to_string();
        *self = new;
        return Ok(self.clone());
    }
}
impl VariableCompletion for PathBuf {
    fn required_variables(&self) -> Result<Vec<String>> {
        self.to_str()
            .context("Can not parse path as String")?
            .to_string()
            .required_variables()
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<PathBuf> {
        let mut string = self
            .to_str()
            .context("Can not parse path as String")?
            .to_string();
        string.set_single_variable(key, value)?;
        *self = PathBuf::from(string);
        Ok(self.clone())
    }
}

impl VariableCompletion for FileSource {
    fn required_variables(&self) -> Result<Vec<String>> {
        match self {
            FileSource::Archive { archive, path } => {
                let rb_archive = archive.to_owned().required_variables()?;
                let rb_path = path.to_owned().required_variables()?;
                Ok(vecset(vec![rb_archive, rb_path]))
            }
            FileSource::Borg {
                archive,
                backup_id,
                path,
            } => {
                let rb_archive = archive.to_owned().required_variables()?;
                let rb_path = path.to_owned().required_variables()?;
                let rb_bid = backup_id.to_owned().required_variables()?;
                Ok(vecset(vec![rb_archive, rb_path, rb_bid]))
            }
            FileSource::Download { url } => url.clone().required_variables(),
            FileSource::Git { repo, commit, path } => {
                let rb_path = path.to_owned().required_variables()?;
                let rb_repo = repo.to_owned().required_variables()?;
                let rb_commit = commit.to_owned().required_variables()?;
                Ok(vecset(vec![rb_repo, rb_commit, rb_path]))
            }
            FileSource::Local { path } => path.to_owned().required_variables(),
            FileSource::Text { content ,ignore_variables} => {
                if *ignore_variables{
                    Ok(vec!())
                }else{
                content.to_owned().required_variables()}
            },
        }
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<FileSource> {
        *self = match self {
            FileSource::Archive { archive, path } => FileSource::Archive {
                archive: archive.set_single_variable(key, value)?,
                path: path.set_single_variable(key, value)?,
            },
            FileSource::Borg {
                archive,
                backup_id,
                path,
            } => FileSource::Borg {
                archive: archive.set_single_variable(key, value)?,
                backup_id: backup_id.set_single_variable(key, value)?,
                path: path.set_single_variable(key, value)?,
            },
            FileSource::Download { url } => FileSource::Download {
                url: url.set_single_variable(key, value)?,
            },
            FileSource::Git { repo, commit, path } => FileSource::Git {
                repo: repo.set_single_variable(key, value)?,
                commit: commit.set_single_variable(key, value)?,
                path: path.set_single_variable(key, value)?,
            },
            FileSource::Local { path } => FileSource::Local {
                path: path.set_single_variable(key, value)?,
            },
            FileSource::Text { content ,ignore_variables} => {
                if *ignore_variables{
                    FileSource::Text {
                        content:content.clone(),ignore_variables:*ignore_variables
                    }
                }else{
                FileSource::Text {
                
                content: content.set_single_variable(key, value)?,ignore_variables:*ignore_variables
            }}},
        };
        return Ok(self.clone());
    }
}

impl<T> VariableCompletion for Vec<T>
where
    T: VariableCompletion,
{
    fn required_variables(&self) -> Result<Vec<String>> {
        let mut req = vec![];
        for v in self {
            req.push(v.required_variables()?);
        }
        Ok(vecset(req))
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        let mut new = vec![];
        for mut v in self.clone() {
            new.push(v.set_single_variable(key, value)?);
        }
        *self = new;
        return Ok(self.clone());
    }
}

impl VariableCompletion for File {
    fn required_variables(&self) -> Result<Vec<String>> {
        let rb_path = self.path.required_variables()?;
        let rb_sources = self.sources.required_variables()?;
        Ok(vecset(vec![rb_path, rb_sources]))
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        Ok(File {
            path: self.path.set_single_variable(key, value)?,
            tags: self.tags.clone(),
            hash: self.hash.clone(),
            sources: self.sources.set_single_variable(key, value)?,
        })
    }
}

fn vecset<T: Clone + Eq + std::hash::Hash>(vecs: Vec<Vec<T>>) -> Vec<T> {
    let mut union_set: HashSet<T> = HashSet::new();
    for vec in vecs {
        for element in vec {
            union_set.insert(element);
        }
    }
    union_set.into_iter().collect()
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_var() {
        let mut str = "the var is {{varname}}".to_string();
        assert_eq!(
            "the var is varvalue".to_string(),
            str.set_single_variable("varname", "varvalue").unwrap()
        );
    }
}
