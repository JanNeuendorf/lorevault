use crate::*;

pub trait VariableCompletion: Sized + Clone {
    fn required_variables(&self) -> Result<Vec<String>>;
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self>;
    fn set_variables(&self, map: &HashMap<String, String>) -> Result<Self> {
        let requested = self.required_variables()?;
        let mut new = self.clone();
        for key in &requested {
            let value = map
                .get(key)
                .context(format!("Required key: {} is not in variables", key))?;
            new.set_single_variable(key, value)?;
        }
        Ok(new)
    }
}

impl VariableCompletion for String {
    fn required_variables(&self) -> Result<Vec<String>> {
        let re = Regex::new(r"\{\{([^{}]+)\}\}")
            .context("Failed to initialize regular expression for variables")?; // This should never happen since the expression is fixed.
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
            FileSource::Auto(s) => s.required_variables(),

            FileSource::Download { url } => url.clone().required_variables(),
            FileSource::Git {
                repo,
                id: commit,
                path,
            } => {
                let rb_path = path.to_owned().required_variables()?;
                let rb_repo = repo.to_owned().required_variables()?;
                let rb_commit = commit.to_owned().required_variables()?;
                Ok(vecset(vec![rb_repo, rb_commit, rb_path]))
            }
            FileSource::Sftp {
                user,
                service,
                path,
                ..
            } => {
                let rb_path = path.to_owned().required_variables()?;
                let rb_user = user.to_owned().required_variables()?;
                let rb_service = service.to_owned().required_variables()?;
                Ok(vecset(vec![rb_service, rb_user, rb_path]))
            }
            FileSource::Local { path } => path.to_owned().required_variables(),
            FileSource::Text {
                content,
                ignore_variables,
            } => {
                if *ignore_variables {
                    Ok(vec![])
                } else {
                    content.to_owned().required_variables()
                }
            }
        }
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<FileSource> {
        *self = match self {
            FileSource::Auto(s) => Self::Auto(s.set_single_variable(key, value)?),
            FileSource::Download { url } => FileSource::Download {
                url: url.set_single_variable(key, value)?,
            },
            FileSource::Git {
                repo,
                id: commit,
                path,
            } => FileSource::Git {
                repo: repo.set_single_variable(key, value)?,
                id: commit.set_single_variable(key, value)?,
                path: path.set_single_variable(key, value)?,
            },
            FileSource::Sftp {
                user,
                service,
                path,
                port,
            } => FileSource::Sftp {
                user: user.set_single_variable(key, value)?,
                service: service.set_single_variable(key, value)?,
                path: path.set_single_variable(key, value)?,
                port: *port,
            },
            FileSource::Local { path } => FileSource::Local {
                path: path.set_single_variable(key, value)?,
            },
            FileSource::Text {
                content,
                ignore_variables,
            } => {
                if *ignore_variables {
                    FileSource::Text {
                        content: content.clone(),
                        ignore_variables: *ignore_variables,
                    }
                } else {
                    FileSource::Text {
                        content: content.set_single_variable(key, value)?,
                        ignore_variables: *ignore_variables,
                    }
                }
            }
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
        let rb_edits = self.edits.required_variables()?;
        Ok(vecset(vec![rb_path, rb_sources, rb_edits]))
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        Ok(File {
            path: self.path.set_single_variable(key, value)?,
            tags: self.tags.clone(),
            hash: self.hash.clone(),
            sources: self.sources.set_single_variable(key, value)?,
            edits: self.edits.set_single_variable(key, value)?,
        })
    }
}

pub fn vecset<T: Clone + Eq + std::hash::Hash>(vecs: Vec<Vec<T>>) -> Vec<T> {
    let mut union_set: HashSet<T> = HashSet::new();
    for vec in vecs {
        for element in vec {
            union_set.insert(element);
        }
    }
    union_set.into_iter().collect()
}

impl VariableCompletion for Inclusion {
    fn required_variables(&self) -> Result<Vec<String>> {
        let rb_subfolder = self.subfolder.required_variables()?;
        let rb_config = self.config.required_variables()?;
        Ok(vecset(vec![rb_subfolder, rb_config]))
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        Ok(Self {
            config: self.config.set_single_variable(key, value)?,
            subfolder: self.subfolder.set_single_variable(key, value)?,
            tags: self.tags.clone(),
            with_tags: self.with_tags.clone(),
            hash: self.hash.clone(),
        })
    }
}

pub fn resolve_variable_inter_refs(
    vars_in: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let mut resolved: HashMap<String, String> = HashMap::new();
    let mut current_resolved_count = 0;
    for _ in 0..1000 {
        // This could be a while loop, but I want to make sure there is no recursive case that is missed.
        for (k, v) in vars_in {
            if v.required_variables()?.len() == 0 {
                resolved.insert(k.clone(), v.clone());
            } else {
                match v.set_variables(&resolved) {
                    Ok(filled) => {
                        resolved.insert(k.clone(), filled.clone());
                    }
                    Err(_) => continue,
                }
            }
        }
        if resolved.len() == current_resolved_count {
            return Err(format_err!(
                "There seems to be some problem with variable inter-reference."
            ));
        } else if resolved.len() == vars_in.len() {
            return Ok(resolved);
        } else {
            current_resolved_count = resolved.len();
        }
    }
    Err(format_err!(
        "There seems to be some problem with variable inter-reference."
    ))
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
    #[test]
    fn test_var_inter_ref() {
        let mut vars_in = HashMap::new();
        vars_in.insert("simple".to_string(), "value".to_string());
        vars_in.insert("complex".to_string(), "plainand{{simple}}".to_string());
        vars_in.insert(
            "more_complex".to_string(),
            "{{simple}} and {{complex}}".to_string(),
        );
        vars_in.insert(
            "even_more_complex".to_string(),
            "{{{{more_complex}}}}".to_string(),
        );

        let vars_out = resolve_variable_inter_refs(&vars_in).unwrap();
        assert_eq!(vars_out.get("simple").unwrap(), "value");
        assert_eq!(vars_out.get("complex").unwrap(), "plainandvalue");
        assert_eq!(
            vars_out.get("more_complex").unwrap(),
            "value and plainandvalue"
        );
        assert_eq!(
            vars_out.get("even_more_complex").unwrap(),
            "{{value and plainandvalue}}"
        );
    }
}
