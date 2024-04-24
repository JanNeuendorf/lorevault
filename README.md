Lorevault is a simple program that creates a folder from a declarative configuration file.
### Motivation                                                                                               
>When I ran that test ten minutes ago, did I forget to to delete the old log files? Is that why it failed?
>
> -- Me, every five minutes

The main motivation for this project is to define folders in a way that can be made completely reproducible. 
This, of course, could also be done by copying a reference folder or cloning a git-repo. 
There are a few problems with this
- You might want to do this after every step of your script, just to make sure nothing changed. This can be costly.
- This gives you no record of what was in this folder.
- Changes to your reference folder are dangerous. Unless you always store it next to your project, you might lose it.
- You might want to test/build with a slightly different folder, forcing you to make and undo changes carefully.

To combat those problems, we can use 

- Hashes to make sure the files are unchanged.
- Support for version control (*git*).
- Multiple sources for a single file to make sure at least one keeps working.
- Tags to conditionally include or change files.

While you can be pedantic, you do not have to be, so you can use this for simple templates as well.


### Installation
You can install lorevault using Cargo.
```bash
cargo install --git https://github.com/JanNeuendorf/lorevault
```

### CLI

The command:
```sh
lorevault sync config.toml targetfolder -t customtag
```
creates the folder according to the recipe. 
If the folder already exists, it is restored to the prescribed state with minimal work.

Other subcommands are `check`, to see which sources are valid, `example` to write out a configuration file, `tags` to list the available tags, and `hash` to get the SHA3-256 of a file.

The configuration file can be read in from a local or remote git-repo with the syntax `repo#commit:path`.

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
It could be a local or remote git-repo:
```toml
[[file.source]]
type = "git"
repo = "https://github.com/some_repo.git"
commit = "fb17a46eb92e8d779e57a10589e9012e9aa5f948"
path = "path/in/repo"
```
Other supported sources are text, URLs and files in archives.
Folders and symbolic links are not supported. 

When using an inline table, we can use the following notation:
```toml
[[file]]
path = "subfolder/my_file"
sources=["/some/path","repo#commit:path","/path/to/archive.tar.xz:file"]
```
The strings are then parsed into other sources. Only local files, archives and git-repos are supported.

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
They can not be used inside hashes, tags, types or other variables.


### Including Configs
We can include other configuration files. 
```toml
[[include]]
config="/path/to/included.toml" # Can be repo#commit:path
subfolder="files/go/here" # Defaults to folder root.
required_tags=["tag1"] # If not set, the file will not be included.
with_tags=["tag2"] # Will be passed to the other file.

```
Variables are not shared between files. Tags for included files can only be activated in the way shown above and are not influenced by the tags activated for the including file.
No files from included configs can replace files defined locally.

### Relative Paths
In general, relative paths are not allowed inside config files.

It might, however, be useful to refer to data stored together with the config. 
This is especially true, if the config is inside a git-repo. 

For this, we can use build-in variables.
If the config file is read from a git-repo, the variables 
`SELF_REPO` and `SELF_COMMIT` are set automatically.
If it is a local file, `SELF_PARENT` is set.
`SELF_ROOT` gives either `repo#commit:` or the parent folder. 

It is therefore a good convention to put the config file in the root of the project, regardles of whether the project is a git-repo or just a local folder. 

Here is an example:
```
project/
│
├── config.toml
│
└─── data/
     └── file.txt
```
In `config.toml`:
```toml
[[file]]
path = "new/filename.txt"
sources=["{{SELF_ROOT}}/data/file.txt"]
```

If the config file is refered to as `repo#commit:path` (from the cli or by inclusion in another config), 
the contents of `new/filename.txt` will match the state of `data/file.txt` at the time of that commit. 
If it is refered to with a path, it is the current version in the folder.

### Details
The folder is always deleted and recreated. This ensures that there are no subtle changes that can be missed. If it existed before, it is used as a reference. If a file has a defined hash and the file in the folder matches it, it can just be taken from there.
This means that if the folder was not changed and all hashes are set, nothing needs to be cloned, downloaded or extracted from archives.

A temporary directory is used to store cloned git-repos. It lives for the time of the command and acts as a cache, so we do not need to clone from the same URL multiple times. 

### Limitations

- It only works on Unix systems. (Only tested on Linux.)
- The contents of the folder are created in memory, so very large files are to be avoided.
- Every file must be named explicitly. There is no way to include entire folders.
- There is no control over metadata/permissions.
- There is no support for authentication when cloning a repo.

### Contributing 

**All contribuitions are very welcome, but most of all this project needs testing.**

There are a few tests in the `justfile` to get started. 
It is, however, very hard to test alone. 









