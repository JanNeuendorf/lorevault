# Lorevault 📜🏦

Lorevault is a simple program that creates a directory from a declarative configuration file.

## Motivation                                                                                               
>When I ran that test ten minutes ago, did I forget to to delete the old log files? Is that why it failed?
>
> -- Me, every five minutes

The main motivation for this project is to define directories in a way that can be made completely reproducible. 
This, of course, could also be done by copying a reference directory or cloning a git-repo. 
There are a few problems with this:
- This gives you no record of how the directory was built.
- Changes to your reference directory are dangerous. Unless you always store it next to your project, you might lose it.
- You might want to test or build with a slightly different directory, forcing you to make and undo changes carefully.

To combat those problems, we can use:
- Hashes to make sure the files are unchanged.
- Version control (git) for individual files or directories.
- Multiple sources for a single file to make sure at least one keeps working.
- Tags to conditionally include or change files.

While you can be pedantic, you do not have to be, so you can use this for simple **templates**.

**This can also be used to manage your dotfiles.** 

## Getting Started
You can install the latest version using Cargo.
```bash
cargo install --git https://github.com/JanNeuendorf/lorevault
```
Then run
```bash
lorevault example
```
to get a basic example.


## CLI 
The command:
```sh
lorevault sync config.toml targetdir --tags=tag1,tag2
```
creates the directory at `targetdir` according to the recipe. 
The directory is always deleted and recreated. This ensures that there are no subtle changes that can be missed. If the directory existed before, it is used as a reference. If a file has a defined hash and the file in the directory matches it, it can be taken from there.

Other subcommands are `tags` to list the available tags, `list` to list the files that would be created, and `hash` to get the SHA3-256 of a file.

The configuration file can be read in from a local or remote git-repo with the syntax `repo#id:path`.
It does not have to be stored in your project's directory.

## Config File
The config file is a `.toml` file that consists of a list of file descriptions. 

### Files
One thing, the config file might include is a list of individual files. 
Here is an example:

```toml
[[file]]
path = "my_subdir/my_file.txt"
hash = "741C077E70E4869ADBC29CCC34B7935B58DDAC16A4B8007AC127181E2148F468"
tags = ["tag1","tag2"]
sources=["/some/local/file.txt","repo#id:path/to/file.txt"]
```
The first variable, `path` defines where the file will be located in the target folder. 
The directory `my_subdir` will be created automatically.

Here, we specified the optional `SHA3-256` hash of the file. This has two advantages: we get an error whenever we are trying to load a file with a wrong hash and we might avoid downloading files if the file already matches the hash. 

We can specify a list of **tags**. The file will then only be included if at least one of the tags is activated. 
It will replace untagged files at the same path. 

The last line specifies a list of possible sources for the file. 
The list is checked in order, so a local copy should be listed first.

There are several kinds of sources:

#### Local Files
You can give a path to a local file, but it must be an absolute path. This ensures that it does not matter, where our config file is stored and from where we call the `sync` command.

#### Files in Git Repos
Using the syntax `repo#id:path`, we can load files from git repositories. They can be local, in which case the path to the repository must be absolute, or remote. 

Remote repos are cloned to a cache directory that persists until the end of the process. This ensures that the same repo is not cloned multiple times. 

The remote path can be *ssh:* `user@machine:repo.git#id:path` or *http:* `https://website.com/repo.git#id:path`.

 Authentication for cloning repos is handled by [auth-git2-rs](https://github.com/de-vri-es/auth-git2-rs), so you can clone private repos with the correct ssh-key. If and how the key is unlocked is up to the user's machine. 


The `id` can be a commit hash, a tag or a branch. When a branch is specified, we get the latest commit to that branch. 

Technically, the repos are not cloned but mirrored. This preserves other branches and their tags, but it is slow. To speed things up, one should add a local clone of the repository to the list of sources. 

Submodules are not supported!

#### URLs
You can give a URL starting with `http` or `https`. It must return a file-response and there is no support for authentication or caching. 

#### Files on a different machine
The syntax `user@machine:some/file` loads the file over sftp. The default port is 22.

#### Text
We can specify the contents of the file as text. For this, we need a slightly different `.toml` syntax:
```toml 
[[file]]
path="my_file.txt"
[[file.source]]
type = "text"
content = """
Hello,
this file was written with Lorevault.
"""
```
The other sources can be written in this way too.

### Edits 
We might want to include a file with a slight modification. 
It would be unfortunate if we had to store the edited copy, especially if we have multiple sources for the original. 
If the file's content is an utf8-encoded string, we can make small edits like this:

```toml
[[file]]
path = "my_dotfile.in"
hash = "741C077E70E4869ADBC29CCC34B7935B58DDAC16A4B8007AC127181E2148F468"
sources=["/some/path","repo#id:path"]
 
[[file.edit]]
type="insert"
content="# The document begins\n\n"
position="prepend" # could be "append" or after a line number.

[[file.edit]]
type="replace"
from="setting=false"
to="setting=true"
tags=["flip"] # Will be skipped if the tag is not active.

[[file.edit]]
type="delete"
start=30 # line numbers (inclusive)
end=100
```
The hash always refers to the hash before any edits are made. Line numbers are counted from 1. The edits are made in sequence, so the line numbers change. 

### Directories

We can include entire directories
```toml 
[[directory]]
path="my_included_directory"
count=5 # optional
sources=["/path/to/dir","repo#id:path/to/dir"]
ignore_hidden=false # This is the default
tags = ["tag1","tag2"]
```
This will try to list the directory and copy all contents to the new directory at `path`.
While the directory can be nested, it can not contain any objects that are not files. This includes empty directories. 
We have the option to specify the expected number of files as a check. The possible sources are local directories and directories in git repos. They work the same as for single files.
The first working source is used for listing the directory and fetching the files. 
In practice, the directory is expanded and the files are added to the list of files individually.

### Variables
To avoid repetition, variables can be set at the beginning of the file and used in the following way:
```toml
var.user = "you"
var.mypath = "subdir/for/{{user}}"

[[file]]
path = "{{mypath}}/file.txt"

[[file.source]]
type = "text"
content = "This file was written by {{user}}."
ignore_variables=false # This is the default. If true, the text is protected.
```
They can not be used inside hashes, tags, types or editing positions.

### Including Configs
We can include other configuration files. 
```toml
[[include]]
config="/path/to/included.toml" # Can be repo#id:path
subdir="files/go/here" # Defaults to directory root.
required_tags=["tag1"] # If not set, the file will not be included.
with_tags=["tag2"] # Will be passed to the other file.

```
Variables are not shared between files. Tags for included files can only be activated in the way shown above and are not influenced by the tags activated on the CLI.

 You can specify the hash of the included `.toml` file itself.


The behavior should be the same as building the directory with the required tags first and then including it. 

### Relative Paths
In general, relative paths are not allowed inside config files.

It might, however, be useful to refer to data stored together with the config. 
This is especially true if the config is inside a repository. 

For this, we can use built-in variables.
If the config file is read from a git-repo, the variables 
`SELF_REPO` and `SELF_ID` are set automatically.
If it is a local file, `SELF_PARENT` is set.
`SELF_ROOT` gives either `repo#id:` or the parent directory. 

It is therefore a good convention to put the config file in the root of the project, regardless of whether the project is a git-repo or just a local directory. 

Here is an example:
```
project/
│
├─── config.toml
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

If the config file is referred to as `repo#commit:config.toml` (from the CLI or by inclusion in another config), 
the contents of `new/filename.txt` will match the state of `data/file.txt` at the time of that commit. 
If it is referred to with a path, it is the current version in the directory.

## Partially Managing a Directory
Sometimes we do not want to control the entire directory. A good example might be managing **dotfiles** in `~/.config`. 
Resetting the entire directory is probably not what we want. Maybe the configuration files for some programs are managed in some other way. 

To only update parts of the directory we can use:
```sh
lorevault sync -S config.toml target_dir
```
where the `-S` stands for *skip first level*. 

This will preserve paths that differ from the controlled files at the first level. 

Great, what does that mean? It means that if your `config.toml` creates a file in a specific subdirectory (directly or by import) 
this subdirectory is deleted and recreated according to the config. If no such file exists, the subdirectory (or file) is left as it was. 

Let's walk through an example:

The config file defines the following files (as can be seen with the `list` subcommand).
- `subdir1/subsubdir/file1.txt`
- `file2.txt`

The directory currently looks like this:
```
target_directory/
│
├─── file2.txt
│
├─── file3.txt
│
├─── subdir1/
│    └── file
│
└───subdir2/
     └── file
```


What will happen when running `sync -S` is the following:
- A path starting with `subdir1` is defined in the config file. Therefore, the program assumes that we want to control the entire folder `subdir1`. **It is completely replaced.** It does not matter that we only defined a single file in a subdirectory.
- `file2.txt` is also defined. Therefore the file is replaced.
- `file3.txt` is not defined and there is no file starting with `subdir2/`. These paths are not deleted or changed. 

Unless we use the `-Y` option, we will get a list of all controlled paths for confirmation.


## Dotfile management

On linux you can use the subcommand

```sh
lorevault config config.toml
```
This will find `~/.config` and sync to it with the `-S` option. 







## Limitations

- It only works on Unix systems. (Only tested on Linux.)
- The contents of the directory are created in memory, so very large files are to be avoided.
- There is no control over metadata/permissions.


## Contributing 

**All contributions are very welcome, but most of all this project needs testing.**

There are a few tests in the `justfile` to get started. 
It is, however, very hard to test alone. 
I am thankful for every bug report. You can compile with `--features=debug` and then run the program with the flag `-d` to get additional debug chatter. 



