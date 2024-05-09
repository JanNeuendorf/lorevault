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
                    info!("Retrieved {} from reference.", item.get_path().display());
                    memfolder.0.insert(
                        item.get_path().clone(),
                        item.from_reference_unchecked(&content, tags)?,
                    );
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
