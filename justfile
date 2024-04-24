#This is a justfile. See: https://github.com/casey/just

test_prefix:="cargo run -q" 
exists:="test -e"
absent:="test ! -e"

fmt:
    cargo fmt

clean: test_clean
    -rm -r target

test_clean:
    -rm -r tmpfolder
    - rm lorevault_example.toml

test: fmt
    cargo test
    just test1 test2 test3 test4

build: test 
    cargo build --release

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
    @just test_fails "{{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm -t wrongtag"
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
    @just test_fails "{{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t conflict -t subfolder"
    {{absent}} tmpfolder/shouldnotexist

# Test 3 exists to check if remote repos are cloned only once. 
# It does not fail, but it will take a long time and print Cloning ... over and over.
test3:test_clean 
    {{test_prefix}} sync testing/testconfig3.toml tmpfolder --no-confirm

# Test 4 checks that the other cli commands work
test4:test_clean 
    {{test_prefix}} sync testing/testconfig4.toml tmpfolder --no-confirm
    @just test_fails "{{test_prefix}} check testing/testconfig4.toml" # One source give an invalied hash
    {{test_prefix}} tags testing/testconfig4.toml
    {{test_prefix}} example 
    @diff lorevault_example.toml src/lorevault_example.toml 
    {{test_prefix}} hash tmpfolder/subfolder/fromarchive2.txt


# Check if a folder contains the expected number of items.
count_folder folder expected:
    #!/usr/bin/env python3
    import os
    assert(len(os.listdir("{{folder}}"))=={{expected}})

# Fails if the provided command works.
test_fails command:
    #!/usr/bin/env python3
    import os
    assert(os.system("{{command}}> /dev/null 2>&1")!=0)


