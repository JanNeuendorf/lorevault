use crate::*;
use std::collections::HashMap;

pub struct MemFolder(pub HashMap<PathBuf, Vec<u8>>);

impl MemFolder {
    pub fn empty() -> Self {
        return MemFolder(HashMap::new());
    }

    pub fn load_first_valid_with_ref(
        conf: &Config,
        tags: &Vec<String>,
        reference_memfolder: &Self,
    ) -> Result<Self> {
        let mut memfolder = MemFolder::empty();
        for item in &conf.get_active(tags)? {
            if let (Some(reqhash), Some(content)) =
                (&item.hash, reference_memfolder.0.get(item.get_path()))
            {
                let hash = compute_hash(content);
                if &hash == reqhash {
                    memfolder.0.insert(item.get_path().clone(), content.clone());
                }
            } else {
                memfolder.0.insert(
                    item.get_path().clone(),
                    fetch_first_valid(&item.sources, &item.hash)?,
                );
            }
        }

        Ok(memfolder)
    }

    pub fn write_to_folder(&self, out_path: &PathBuf) -> Result<()> {
        if out_path.exists() {
            if out_path.is_dir() {
                fs::remove_dir_all(&out_path)?;
            } else {
                return Err(Error::msg("out file exists, but is not a directory"));
            }
        }

        for (subpath, content) in &self.0 {
            let mut target_path = out_path.clone();
            target_path.push(subpath);
            let prefix = target_path.parent().context("malformed path")?;
            fs::create_dir_all(prefix).context("Path could not be created")?;
            let mut _file = std::fs::File::create(&target_path)?;
            fs::write(target_path, content).context("could not write to file")?;
        }
        Ok(())
    }

    pub fn load_from_folder(folder_path: &PathBuf) -> Result<Self> {
        let file_list = get_files_in_folder(folder_path)?;
        let mut memfolder = Self::empty();
        for relpath in &file_list {
            memfolder
                .0
                .insert(relpath.clone(), fs::read(folder_path.join(relpath))?);
        }
        Ok(memfolder)
    }
}

fn get_files_in_folder(folder_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let full_paths = get_full_paths_in_folder(folder_path)?;
    let mut trimmed = vec![];
    for p in &full_paths {
        let t = p.strip_prefix(folder_path).unwrap();
        trimmed.push(t.to_path_buf());
    }
    Ok(trimmed)
}
fn get_full_paths_in_folder(folder_path: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_file() {
            let file_path = entry.path();
            files.push(file_path);
        } else if file_type.is_dir() {
            let dir_files = get_full_paths_in_folder(&entry.path())?;
            if dir_files.is_empty() {
                return Err(Error::msg("empty folders not supported"));
            }
            files.extend(dir_files);
        } else {
            return Err(Error::msg("only regular files supported"));
        }
    }

    Ok(files)
}
