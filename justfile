#This is a justfile. See: https://github.com/casey/just

test_prefix:="cargo run -q" 
exists:="test -e"
absent:="test ! -e"

fmt:
    cargo fmt

clean: test_clean
    -rm -r target

@test_clean:
    -rm -r tmpfolder
    - rm lorevault_example.toml

test: fmt
    cargo test
    just test1 test2 test3 test4 test5 test6 test7

build: test 
    cargo build --release

install:
    cargo install --path="{{justfile_directory()}}"

# This should result in a statically linked binary.
build_static: test
    cargo build --release --target=x86_64-unknown-linux-musl --features=static


# Test 1 uses a config that includes variables, tags and multiple sources. We then check, if the folder is created as expected.
# This test requires connection to GitHub
test1:test_clean 
    {{test_prefix}} check testing/testconfig1.toml 
    {{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm
    # Run again to make sure it works when the folder exists
    {{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm
    {{exists}} tmpfolder/description.txt
    {{exists}} tmpfolder/rustlings_readme.md
    @just count_folder tmpfolder 3
    @just count_folder tmpfolder/subfolder 2
    {{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm -t file2
    # It should fail if the tag was not defined in the config
    @just error_contains "{{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm -t wrongtag" "The tag wrongtag is not defined"
    @just count_folder tmpfolder 3
    @just count_folder tmpfolder/subfolder 3

# Test 2 checks if the inclusion of other config files is handled correctly.
test2:test_clean 
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm
    @just count_folder tmpfolder 2
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t subfolder
    @just count_folder tmpfolder 3
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm
    @just count_folder tmpfolder 2
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t conflict
    @just count_folder tmpfolder 3
    # While the tags "subfolder" and "conflict" work in separation, their paths conflict.
    @just error_contains "{{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t conflict -t subfolder" "There are two files for"
    {{absent}} tmpfolder/shouldnotexist

# Test 3 exists to check if remote repos are cloned only once. 
# It does not fail, but it will take a long time and print Cloning ... over and over.
test3:test_clean 
    {{test_prefix}} sync testing/testconfig3.toml tmpfolder --no-confirm

# Test 4 checks that the other cli commands work
test4:test_clean 
    {{test_prefix}} sync testing/testconfig4.toml tmpfolder --no-confirm
    @just error_contains "{{test_prefix}} check testing/testconfig4.toml" "Hash did not match" 
    {{test_prefix}} tags testing/testconfig4.toml
    {{test_prefix}} example 
    @just error_contains "{{test_prefix}} example" "already exists"
    @diff lorevault_example.toml src/lorevault_example.toml 
    {{test_prefix}} hash tmpfolder/subfolder/fromarchive2.txt
    {{test_prefix}} list testing/testconfig2.toml
    {{test_prefix}} list testing/testconfig2.toml -t conflict

# Test 5 checks multiple configurations that have something wrong with them.
test5:test_clean 
    just error_contains "{{test_prefix}} sync testing/failure1.toml tmpfolder/ -t inc" "two files for path included/main.rs"
    just error_contains "{{test_prefix}} sync testing/failure2.toml tmpfolder/" "relative path"
    just error_contains "{{test_prefix}} sync testing/failure3.toml tmpfolder/ -t inc" "two files for path included/main.rs"

# Test 6 checks that a file which is included from another config without a tag can be replaced by a local file
test6:test_clean
    {{test_prefix}} sync testing/testconfig6.toml tmpfolder --no-confirm
    diff tmpfolder/included/main.rs src/main.rs
    {{test_prefix}} sync testing/testconfig6.toml tmpfolder --no-confirm -t overwrite
    diff tmpfolder/included/main.rs src/cli.rs

# Test 7 checks if variables reference other variables correctly.
test7:test_clean
    {{test_prefix}} sync testing/testconfig7.toml tmpfolder --no-confirm
    {{exists}} tmpfolder/subfolder/my_value/subsubfolder/file.txt

# Check if a folder contains the expected number of items.
count_folder folder expected:
    #!/usr/bin/env python3
    import os
    assert(len(os.listdir("{{folder}}"))=={{expected}})

error_contains command msg:
    #!/usr/bin/env python3
    import subprocess
    result = subprocess.run("{{command}}", shell=True, capture_output=True, text=True)
    if (result.returncode==0):
        print("Command refused to fail")
        exit(1)

    if ("{{msg}}" not in result.stderr):
        print("Wrong error message:")
        print(result.stderr)
        exit(1)

           
