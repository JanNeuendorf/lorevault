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
lorevault config.toml targetfolder -t customtag
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
We can specify a local or remote git-repo:
```toml
[[file.source]]
type = "git"
repo = "https://github.com/some_repo.git"
commit = "fb17a46eb92e8d779e57a10589e9012e9aa5f948"
path = "path/in/repo"
```
Other supported sources are text, URLs and files in archives.
Folders and symbolic links are not supported. 

When using an inline table, we can use the following notation
```toml
[[file]]
path = "subfolder/my_file"
sources=["/some/path","repo#commit:path","/path/to/archive.tar:file]
```
The strings are then parsed into other sources. Currently, only local files and git-repos are supported.

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
Variables are not shared between files. Tags for included files can only be activated in the way shown above and are not influenced by the tags activated for the including file. 


### Limitations

- The contents of the folder are created in memory, so very large files are to be avoided.
- Every file must be named explicitly. There is no support for including folders.
- There is no control over metadata/permissions.






