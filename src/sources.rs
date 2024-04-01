use anyhow::{Context, format_err, Result};

use git2::{Oid, Repository};
use reqwest;
use sha3::{Digest, Sha3_256};
use std::path::PathBuf;
use std::{fs, io::Read};
use tar::Archive;
use tempfile::TempDir;
use xz2::read::XzDecoder;
use colored::*;
use std::process::Command;

use std::io::Cursor;
use zip::read::ZipArchive;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FileSource {
    #[serde(rename = "file")]
    //forbid relative paths!!!!!!!!!
    Local { path: PathBuf },
    #[serde(rename = "http")]
    Download { url: String },
    #[serde(rename = "git")]
    Git {
        repo: String,
        commit: String,
        path: PathBuf,
    },
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "archive")]
    Archive { archive: PathBuf, path: PathBuf },
    #[serde(rename = "borg")]
    Borg { archive: PathBuf, backup_id:String,path: PathBuf },
}
impl FileSource {
    pub fn fetch(&self) -> Result<Vec<u8>> {
        match self {
            FileSource::Local { path } => {
                if path.is_relative() {
                    return Err(format_err!("Relative paths are not allowed!"));
                }
                fs::read(path).context("could not read local file")
            }
            FileSource::Download { url } => {
                let response = reqwest::blocking::get(url)?;
                let bytes = response.error_for_status()?.bytes()?.to_vec();
                Ok(bytes)
            }
            FileSource::Git { repo, commit, path } => get_git_file(commit, path, repo),
            FileSource::Text { content } => Ok(content.clone().into_bytes()),
            FileSource::Borg { archive, backup_id, path }=>{read_from_borg(archive, backup_id, path)}
            FileSource::Archive { archive, path } => {
                let filename = archive.file_name().context("no ending")?.to_string_lossy();
                if filename.ends_with(".zip") {
                    extract_file_from_zip(archive, path)
                } else if filename.ends_with(".tar") {
                    extract_file_from_tar(archive, path)
                } else if filename.ends_with(".tar.xz") {
                    extract_file_from_xz_tar(archive, path)
                } else {
                    Err(format_err!("Unsupported archive type (ending)"))
                }
            }
        }
    }
}

pub fn fetch_first_valid(sources: &Vec<FileSource>, hash: &Option<String>) -> Result<Vec<u8>> {
    for s in sources {
        let result = s.fetch();

        if result.is_ok() {
            if hash.is_none() {
                return result;
            } else {
                if hash.as_ref().expect("must be some")
                    == &compute_hash(&result.as_ref().expect("ref must exist"))
                {
                    return result;
                }
            }
        } else {

            let warn=format!("Invalid source {:?}", &s);
            println!("{}",warn.red());
        }
    }
    return Err(format_err!("No valid source in list"));
}

pub fn compute_hash(content: &Vec<u8>) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(content);

    let result = hasher.finalize();
    let hex_string: String = result
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join("");
    return hex_string;
}

fn get_git_file(commit_hash: &str, file_path: &PathBuf, repo_path: &str) -> Result<Vec<u8>> {
    // Open the remote repository

    let repo: Repository;
    if is_url(repo_path) {
        // If repo_path is a URL, clone the repository into a temporary directory

        repo = clone_repository(repo_path)?;
    } else {
        // If repo_path is a local path, open the repository
        repo = Repository::open(repo_path)?;
    }

    // Get the commit corresponding to the given commit hash
    let commit = repo.find_commit(Oid::from_str(commit_hash)?)?;

    // Get the tree of the commit
    let tree = commit.tree()?;

    // Get the blob corresponding to the file path
    let blob = tree
        .get_path(&std::path::Path::new(file_path))?
        .to_object(&repo)?;

    // Ensure the object is a blob
    if let Some(blob) = blob.as_blob() {
        Ok(blob.content().to_vec())
    } else {
        Err(format_err!("Git object is not a blob"))
    }
}

fn is_url(path: &str) -> bool {
    path.to_string().starts_with("http://") || path.to_string().starts_with("https://")
}

fn clone_repository(repo_url: &str) -> Result<Repository> {
    // Create a temporary directory to clone the repository into
    let temp_dir = TempDir::new()?;

    let repo = git2::build::RepoBuilder::new().clone(repo_url, temp_dir.path())?;

    Ok(repo)
}

fn extract_file_from_zip(path_to_zip: &PathBuf, sub_path: &PathBuf) -> Result<Vec<u8>> {
    // Create a zip archive from the provided zip data
    
    let zip_data = fs::read(path_to_zip)?;
    
    let reader = Cursor::new(zip_data);
    let mut zip = ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut entry =zip.by_index(i)?;
    

        let entry_subpath=strip_first_level(entry.name());


                if entry_subpath==sub_path.to_str().context("invalid path")? {
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content)?;
                    return Ok(content);
                }

    }

    Err(format_err!("file {} not found in {}",sub_path.to_string_lossy(),path_to_zip.to_string_lossy()))
}

fn extract_file_from_tar(archive_path: &PathBuf, file_path: &PathBuf) -> Result<Vec<u8>> {
    extract_file_from_tar_data(&fs::read(archive_path)?, file_path)
}

fn extract_file_from_xz_tar(archive_path: &PathBuf, file_path: &PathBuf) -> Result<Vec<u8>> {
    // Open the XZ-compressed tar file
    let file = fs::File::open(archive_path)?;
    let mut xz = XzDecoder::new(file);

    // Create a buffer to hold the decompressed data
    let mut buf = Vec::new();
    xz.read_to_end(&mut buf)?;
    

    extract_file_from_tar_data(&buf, file_path)
}

fn extract_file_from_tar_data(buf: &Vec<u8>, file_path: &PathBuf) -> Result<Vec<u8>> {
    // Create a cursor from the decompressed data
    let mut cursor = Cursor::new(buf);

    // Create a tar archive from the decompressed data
    let mut archive = Archive::new(&mut cursor);
    
    
    // Iterate through the entries in the tar archive
    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?;
       
        // Convert the entry path to a string
        let entry_path_str = entry_path.to_string_lossy();

        // Check if the entry path matches the file path we're looking for
        if strip_first_level(&entry_path_str) == file_path.to_string_lossy() {
            // Read and return the contents of the entry
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            return Ok(content);
        }
    }

    Err(format_err!("Path not found in tar data"))
}


fn strip_first_level(s:&str) -> String {

        let mut components = s.split('/').collect::<Vec<_>>();

        if components.len() > 1 {

            components.remove(0);


            let stripped_path = components.join("/");


            return stripped_path;
        }else{s.to_string()}
    }


fn read_from_borg(archive_path:&PathBuf,backup_name:&str,sub_path: &PathBuf)->Result<Vec<u8>>{
    let borg_exists=Command::new("borg").arg("-V").output()?.status.success();
    if !borg_exists{
        return Err(format_err!("borg might not be installed"))
    }
    let backup=&format!("{}::{}",archive_path.to_str().context("path not printable")?,backup_name);
    let output = Command::new("borg")
                     .arg("extract")
                     .arg(backup).arg(sub_path.to_str().context("subpath not printable")?)
                     .arg("--stdout")
                     .output()?;
    if !output.status.success(){
        return Err(format_err!("Call to borg failed"))
    }
    return Ok(output.stdout)
}

