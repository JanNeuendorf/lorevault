use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum FileEdit {
    Replace {
        replace_from: String,
        to: String,
        #[serde(default)]
        optional: bool,
    },
}

impl FileEdit {
    pub fn apply(&self, input: impl AsRef<str>) -> Result<String> {
        let str = input.as_ref();
        match &self {
            Self::Replace {
                replace_from: from,
                to,
                optional,
            } => {
                if !*optional && !str.contains(from) {
                    Err(format_err!(
                        "Replacement {} was required but not found",
                        from
                    ))
                } else {
                    Ok(str.replace(from, to))
                }
            }
        }
    }
}
