use crate::*;
pub struct MemFolder(pub HashMap<PathBuf, Vec<u8>>);

impl MemFolder {
    pub fn empty() -> Self {
        return MemFolder(HashMap::new());
    }

    pub fn load_first_valid_with_ref(
        conf: &Config,
        tags: &Vec<String>,
        reference: &PathBuf,
    ) -> Result<Self> {
        let mut memfolder = MemFolder::empty();
        for item in &conf.get_active(tags)? {
            if contains_parent_dir(&item.get_path()) {
                return Err(format_err!(
                    "Escaping the current folder (..) is not allowed."
                ));
            }
            let mut ref_path = reference.clone();
            ref_path.push(item.get_path());
            if let (Some(reqhash), Ok(content)) = (&item.hash, fs::read(ref_path)) {
                let hash = compute_hash(&content);
                if &hash == reqhash {
                    memfolder.0.insert(
                        item.get_path().clone(),
                        item.from_reference_unchecked(&content, tags)?,
                    );
                } else {
                    memfolder
                        .0
                        .insert(item.get_path().clone(), item.build(tags)?);
                }
            } else {
                memfolder
                    .0
                    .insert(item.get_path().clone(), item.build(tags)?);
            }
        }

        Ok(memfolder)
    }

    pub fn write_to_folder(&self, out_path: &PathBuf) -> Result<()> {
        if out_path.exists() {
            if out_path.is_dir() {
                fs::remove_dir_all(&out_path).context(format!(
                    "Could not remove the directory {}.",
                    out_path.display()
                ))?;
            } else {
                return Err(format_err!(
                    "Path {} exists, but it is not a directory.",
                    out_path.display()
                ));
            }
        }
        fs::create_dir(out_path)
            .context("Could not create output folder. Maybe its parent does not exist?")?;

        self.write_into(out_path)?;
        Ok(())
    }

    pub fn write_to_folder_skip_first(&self, out_path: &PathBuf) -> Result<()> {
        if out_path.exists() {
            if out_path.is_dir() {
                for tracked in self.tracked_subpaths()? {
                    let mut tracked_path = out_path.clone();
                    tracked_path.push(tracked);

                    if !tracked_path.exists() {
                        continue;
                    }
                    if tracked_path.is_dir() {
                        fs::remove_dir_all(&tracked_path).context(format!(
                            "Could not remove directory {}.",
                            tracked_path.display()
                        ))?;
                    } else if tracked_path.is_file() {
                        fs::remove_file(&tracked_path).context(format!(
                            "Could not remove file {}.",
                            tracked_path.display()
                        ))?;
                    } else {
                        return Err(format_err!(
                            "Item at {} is not a file or directory.",
                            tracked_path.display()
                        ));
                    }
                }
            } else {
                return Err(format_err!(
                    "Path {} exists, but it is not a directory.",
                    out_path.display()
                ));
            }
        } else {
            fs::create_dir(out_path)
                .context("Could not create output folder. Maybe its parent does not exist?")?;
        }

        self.write_into(out_path)?;
        Ok(())
    }

    fn write_into(&self, out_path: &PathBuf) -> Result<()> {
        for (subpath, content) in &self.0 {
            let mut target_path = out_path.clone();
            let subpath = format_subpath(subpath);
            target_path.push(subpath);
            let prefix = target_path.parent().context("Malformed path")?;
            fs::create_dir_all(prefix).context("Path could not be created")?;
            let mut _file = std::fs::File::create(&target_path)?;
            fs::write(target_path, content).context("Could not write file")?;
        }
        Ok(())
    }

    pub fn tracked_subpaths(&self) -> Result<Vec<PathBuf>> {
        let mut firsts = vec![];
        for k in self.0.keys() {
            let mut path = PathBuf::new();
            path.push(k.components().next().context("Empty path")?);
            if firsts.contains(&path) {
                continue;
            }
            firsts.push(path);
        }
        Ok(firsts)
    }

    #[allow(unused)]
    pub fn size_in_bytes(&self) -> usize {
        self.0.values().map(|v| v.len()).sum()
    }
}
fn contains_parent_dir(path: &PathBuf) -> bool {
    path.components().any(|component| match component {
        std::path::Component::ParentDir => true,
        _ => false,
    })
}
