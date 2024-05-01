use crate::*;
use serde::{Deserialize, Serialize};
const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type")]
pub enum FileEdit {
    #[serde(rename = "replace")]
    Replace {
        from: String,
        to: String,
        #[serde(default = "default_true")]
        required: bool,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        ignore_variables: bool,
    },
    #[serde(rename = "insert")]
    Insert {
        content: String,
        position: EditPosition,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        ignore_variables: bool,
    },
    #[serde(rename = "delete")]
    Delete {
        start: usize,
        end: usize,
        #[serde(default)]
        tags: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]

pub enum EditPosition {
    #[serde(rename = "append", alias = "end")]
    Append,
    #[serde(rename = "prepend", alias = "start")]
    Prepend,
    #[serde(rename = "at_line", untagged)]
    Line(usize),
}

impl FileEdit {
    pub fn apply(&self, input: impl AsRef<str>) -> Result<String> {
        let str = input.as_ref();
        match &self {
            Self::Replace {
                from, to, required, ..
            } => {
                if *required && !str.contains(from) {
                    Err(format_err!(
                        "Replacement {} was required but not found",
                        from
                    ))
                } else {
                    Ok(str.replace(from, to))
                }
            }
            Self::Insert {
                content, position, ..
            } => match position {
                EditPosition::Append => Ok(format!("{}{}", str, content)),
                EditPosition::Prepend => Ok(format!("{}{}", content, str)),
                EditPosition::Line(ln) => {
                    let mut lines: Vec<&str> = str.lines().collect();

                    if *ln > lines.len() {
                        return Err(format_err!("Not enough lines to insert after line {}", ln));
                    }

                    lines.insert(*ln, content);

                    Ok(lines.join("\n"))
                }
            },
            Self::Delete { start, end, .. } => {
                let lines: Vec<&str> = str.lines().collect();
                if *start == 0 || *end == 0 {
                    return Err(format_err!("Line positions are counted from 1."));
                }
                let (start, end) = (start - 1, end - 1);
                let mut new: Vec<&str> = vec![];
                if start > end || end >= lines.len() {
                    return Err(format_err!(
                        "Invalid deletion range: {} {}",
                        start + 1,
                        end + 1
                    ));
                }
                for i in 0..start {
                    new.push(lines.get(i).context("Line not in range")?);
                }
                for i in end + 1..lines.len() {
                    new.push(lines.get(i).context("Line not in range")?);
                }
                return Ok(new.join("\n"));
            }
        }
    }
    pub fn get_tags(&self) -> &Vec<String> {
        match self {
            Self::Replace { tags, .. } => tags,
            Self::Insert { tags, .. } => tags,
            Self::Delete { tags, .. } => tags,
        }
    }
    fn without_tags(&self) -> FileEdit {
        match self {
            Self::Replace {
                from,
                to,
                required,
                ignore_variables,
                ..
            } => Self::Replace {
                from: from.clone(),
                to: to.clone(),
                tags: vec![],
                required: *required,
                ignore_variables: *ignore_variables,
            },
            Self::Insert {
                content,
                position,
                ignore_variables,
                ..
            } => Self::Insert {
                content: content.clone(),
                position: position.clone(),
                tags: vec![],
                ignore_variables: *ignore_variables,
            },
            Self::Delete { start, end, .. } => Self::Delete {
                start: *start,
                end: *end,
                tags: vec![],
            },
        }
    }

    pub fn is_active(&self, tags: &Vec<String>) -> bool {
        if self.get_tags().len() == 0 {
            true
        } else {
            for t in self.get_tags() {
                //print!("checking {} against {:?}",t,tags);
                if tags.contains(t) {
                    return true;
                }
            }
            // print!("inactive {:?}",self);
            return false;
        }
    }
}

pub fn include_edits(edits: &Vec<FileEdit>, tags: &Vec<String>) -> Vec<FileEdit> {
    let mut new: Vec<FileEdit> = vec![];
    for e in edits {
        if e.is_active(tags) {
            new.push(e.without_tags());
        }
    }
    new
}

impl VariableCompletion for FileEdit {
    fn required_variables(&self) -> Result<Vec<String>> {
        match self {
            Self::Replace {
                from,
                to,
                ignore_variables,
                ..
            } => {
                if *ignore_variables {
                    return Ok(vec![]);
                }
                let rb_from = from.required_variables()?;
                let rb_to = to.required_variables()?;
                Ok(vecset(vec![rb_from, rb_to]))
            }
            Self::Insert {
                content,
                ignore_variables,
                ..
            } => {
                if *ignore_variables {
                    Ok(vec![])
                } else {
                    content.required_variables()
                }
            }
            Self::Delete { .. } => Ok(vec![]),
        }
    }
    fn set_single_variable(&mut self, key: &str, value: &str) -> Result<Self> {
        match self {
            Self::Replace {
                from,
                to,
                required: optional,
                tags,
                ignore_variables,
            } => Ok(Self::Replace {
                from: from.set_single_variable(key, value)?,
                to: to.set_single_variable(key, value)?,
                required: *optional,
                tags: tags.clone(),
                ignore_variables: *ignore_variables,
            }),
            Self::Insert {
                content,
                position,
                tags,
                ignore_variables,
            } => Ok(Self::Insert {
                content: content.set_single_variable(key, value)?,
                position: position.clone(),
                tags: tags.clone(),
                ignore_variables: *ignore_variables,
            }),
            Self::Delete { .. } => Ok(self.clone()),
        }
    }
}
