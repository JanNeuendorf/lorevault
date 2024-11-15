use crate::*;
use toml_edit::{array, value, DocumentMut, Table};

fn get_valid_file_hash(f: &File) -> Result<String> {
    if f.hash.is_some() {
        return Err(format_err!("The file already has a hash"));
    }
    let data = fetch_first_valid(&f.sources, &None)?;
    return Ok(compute_hash(&data));
}

fn get_valid_include_hash(inc: &Inclusion) -> Result<String> {
    if inc.hash.is_some() {
        return Err(format_err!("The inclusion already has a hash"));
    }
    let config_data = cli::source_from_string_simple(&inc.config)?.fetch()?;
    return Ok(compute_hash(&config_data));
}

fn get_valid_dir_hash(dir: &Directory) -> Result<Vec<String>> {
    if dir.hash.is_some() {
        return Err(format_err!("The Directory already has a hash"));
    }
    let (dirsource, list) = list_first_valid(&dir.sources)?;
    let mut hashes = vec![path_list_hash(&list)?];
    for path in &sorted_path_list(&list) {
        let filesource = dirsource.get_single_file_source(path)?;
        let file_hash = compute_hash(&filesource.fetch()?);
        hashes.push(file_hash);
    }
    Ok(hashes)
}

fn get_fully_hashed_config(conf: &Config, source: &FileSource) -> Result<Config> {
    let mut new_config = conf.clone();
    for f in &mut new_config.content {
        if f.hash.is_some() {
            continue;
        } else {
            f.hash = Some(get_valid_file_hash(&f)?);
        }
    }
    for inc in &mut new_config.inclusions {
        if inc.hash.is_some() {
            continue;
        } else {
            inc.hash = Some(get_valid_include_hash(&inc)?);
        }
    }
    for dir in &mut new_config.directories {
        if dir.hash.is_some() {
            continue;
        } else {
            dir.hash = Some(get_valid_dir_hash(&dir)?);
        }
    }

    Ok(new_config)
}

pub fn build_locked_toml(source: &FileSource) -> Result<String> {
    let original_string = String::from_utf8(source.fetch()?)?;
    let hashed_config =
        get_fully_hashed_config(&Config::from_filesource(source, true, None)?, &source)?;

    let mut doc: DocumentMut = original_string.parse::<DocumentMut>()?;
    for (file_index, hashed_file) in hashed_config.content.iter().enumerate() {
        let hash = hashed_file
            .hash
            .clone()
            .context("The hash was not precomputed properly")?;
        let table: &mut Table = doc
            .get_mut("file")
            .context("toml edit error")?
            .get_mut(file_index)
            .context("indices of files do not match")?
            .as_table_mut()
            .context("toml entry is malformed")?;
        table.insert("hash", value(hash));
    }
    for (dir_index, hashed_dir) in hashed_config.directories.iter().enumerate() {
        let hash = hashed_dir
            .hash
            .clone()
            .context("The hash was not precomputed properly")?;
        let mut arr = toml_edit::Array::new();
        for subhash in hash {
            arr.push(subhash);
        }
        let table: &mut Table = doc
            .get_mut("directory")
            .context("toml edit error")?
            .get_mut(dir_index)
            .context("indices of directories do not match")?
            .as_table_mut()
            .context("toml entry is malformed")?;
        table.insert("hash", toml_edit::Item::Value(toml_edit::Value::Array(arr)));
    }
    for (inc_index, hashed_inc) in hashed_config.inclusions.iter().enumerate() {
        let hash = hashed_inc
            .hash
            .clone()
            .context("The hash was not precomputed properly")?;
        let included_config = Config::from_general_path(&hashed_inc.config, false, Some(&hash))?;
        if !included_config.is_locked() {
            return Err(format_err!("included config is not locked"));
        }
        let table: &mut Table = doc
            .get_mut("include")
            .context("toml edit error")?
            .get_mut(inc_index)
            .context("indices of includes do not match")?
            .as_table_mut()
            .context("toml entry is malformed")?;
        table.insert("hash", value(hash));
        table.remove("enforce_locked");
        table.insert("enforce_locked", value(true));
    }

    Ok(doc.to_string())
}

#[cfg(test)]
mod test {

    use super::*;
    use toml_edit::{value, DocumentMut};

    #[test]
    fn toml_edit_test() {
        let source = FileSource::Local {
            path: PathBuf::from("src/lorevault_example.toml")
                .canonicalize()
                .unwrap(),
        };
        let string = build_locked_toml(&source).unwrap();
        println!("{}", string);
    }
}
