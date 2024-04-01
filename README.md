Lorevault is a simple program that creates a folder from a declarative configuration file. 

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
This file will only be in the folder, if one of the tags is given. 
It will replace untagged files at the same path.

### CLI

The command:
```sh
lorevault sync config.toml my_folder -t customtag
```
creates the folder according to the recipe. 
If the folder already exists, it is restored to the prescribed state with minimal work.

Other subcommands are `check`, to see which sources are valid, and `hash` to get the SHA3-256 of a file.













