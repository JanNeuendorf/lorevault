Lorevault is a simple program that creates a folder from a declarative configuration file. 

### Installation
You can install lorevault using Cargo.
```bash
cargo install --git https://github.com/JanNeuendorf/lorevault
```

### Config File
The config file is a `.toml` file that consists of a list of file descriptions. 

```toml
[[file]]
path = "subfolder/my_file"
hash = "741C077E70E4869ADBC29CCC34B7935B58DDAC16A4B8007AC127181E2148F468"
```
This is the relative path of the file in the folder. The parent folder is created automatically.
We can specify the SHA3-256 hash of the file.

Then, we list sources for the file. The list is checked in order, so a local copy should be listed first.
It could be a local file:
```toml
[[file.source]]
type = "file"
path = "/home/some_path/local_copy" # It must be an absolute path
```
We can specify a local or remote git-repo:
```toml
[[file.source]]
type = "git"
repo = "https://github.com/some_repo.git"
commit = "fb17a46eb92e8d779e57a10589e9012e9aa5f948"
path = "path/in/repo"
```
Other supported sources are text, URLs, files in archives and Borg backups.
Folders and symbolic links are not supported. 

### Tags
Tags can be specified for conditional inclusion of files.

```toml
[[file]]
path = "subfolder/my_file"
hash = "741C077E70E4869ADBC29CCC34B7935B58DDAC16A4B8007AC127181E2148F468"
tags = ["tag1","tag2"]
```
This file will only be in the folder if one of the tags is given. 
It will replace untagged files at the same path.

### Variables
To avoid repetition, variables can be set in the beginning of the file and used in the following way:
```toml
var.author = "your name"
var.mypath = "some/sub/folder"

[[file]]
path = "{{mypath}}//file.txt"

[[file.source]]
type = "text"
content = "This file was written by {{author}}."
ignore_variables=false # This is the default. If true, the text is protected.
```
They can not be used inside hashes, tags or types.

If the config file is read from a git-repo, the variables 
`SELF_REPO` and `SELF_COMMIT` are set automatically.
This allows references to files from the same commit. If it is a local file, `SELF_PARENT` is set.
`SELF_ROOT` gives either `repo#commit:` or the parent folder.

### Including Configs
We can include other configuration files. 
```toml
[[include]]
config="included.toml" # Can be repo#commit:path
subfolder="files/go/here" # Defaults to folder root.
required_tags=["tag1"] # If not set, the file will not be included.
with_tags=["tag2"] # Will be passed to the other file.

```


### CLI

The command:
```sh
lorevault sync config.toml my_folder -t customtag
```
creates the folder according to the recipe. 
If the folder already exists, it is restored to the prescribed state with minimal work.

Other subcommands are `check`, to see which sources are valid, `example` to write out a configuration file, `tags` to list the available tags, and `hash` to get the SHA3-256 of a file.

The configuration file can be read in from a local or remote git-repo with the syntax `repo#commit:path`.

### Limitations

The contents of the folder are created in memory, so very large files are to be avoided. 











