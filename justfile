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
    -rm lorevault_example.toml
    -rm -rf testing/testrepo

test: fmt
    cargo test
    just example_test bigtest1 bigtest2 bigtest3 failure_tests edits_test 

build: test 
    cargo build --release

install:
    cargo install --path="{{justfile_directory()}}"

install_debug:
    cargo install --path="{{justfile_directory()}}" --features=debug

# This should result in a statically linked binary. Sometimes musl builds fail the first time.
build_musl: test
    -cargo build --release --target=x86_64-unknown-linux-musl 
    cargo build --release --target=x86_64-unknown-linux-musl 
    just output_contains "ldd target/x86_64-unknown-linux-musl/release/lorevault" "statically linked"


release:
    #!/bin/bash
    cargo_pkgid=$(cargo pkgid)
    crate_id=$(echo "$cargo_pkgid" | cut -d "#" -f2)
    name=lorevault-$crate_id-x86_64-linux
    just clean test build build_musl
    cp target/x86_64-unknown-linux-musl/release/lorevault releases/$name
    just output_contains "./releases/$name -V" "$crate_id"


# Check if the example file works.
@example_test: test_clean
    {{test_prefix}} sync src/lorevault_example.toml tmpfolder -Y -t theme 
    just output_contains 'grep Dracula  tmpfolder/Count_Freddy.txt|wc -l' '1'
    {{exists}} "tmpfolder/theme_directory"


@bigtest1:test_clean
    # We start by simply checking that the file works.
    {{test_prefix}} sync testing/bigtest1.toml tmpfolder --no-confirm

    # There should now be three files in the folder.
    just count_folder "tmpfolder" 3

    # Without tags, the main.rs is not our main.rs 
    just error_contains "diff src/main.rs tmpfolder/program_src/main.rs" ""

    # There should be one tag defined in the config file
    just output_contains "{{test_prefix}} tags testing/bigtest1.toml" "replace_main"

    # We activate this tag 
    {{test_prefix}} sync testing/bigtest1.toml tmpfolder --no-confirm -t replace_main

    # Now the main.rs file was replaced with the one from this project.
    just output_contains "diff src/main.rs tmpfolder/program_src/main.rs" ""

    # We replaced "Dracula" with "Freddy"
    just output_contains "grep Freddy tmpfolder/Dracula.txt | wc -l" "41" 

    # The last thing we test is the inclusion of a different config
    {{test_prefix}} sync testing/bigtest1.toml tmpfolder --no-confirm --tags=replace_main,inc 
    just count_folder "tmpfolder" 4
    just output_contains "diff testing/testfolder tmpfolder/included" ""

# This test checks if the -S flag works correctly.
@bigtest2:test_clean
    {{test_prefix}} sync -Y testing/bigtest2.toml tmpfolder 
    just count_folder "tmpfolder" 1

    # We manually create a file
    touch tmpfolder/manfile 
    just count_folder "tmpfolder" 2

    # It should not have been touched.
    {{test_prefix}} sync -SY testing/bigtest2.toml tmpfolder
    just count_folder "tmpfolder" 2

    # If we use regular sync, it should be deleted.
    {{test_prefix}} sync -Y testing/bigtest2.toml tmpfolder
    just count_folder "tmpfolder" 1

    # We create a file in a subfolder the config uses. This should be deleted
    touch tmpfolder/alacritty/manfile 
    {{test_prefix}} sync -SY testing/bigtest2.toml tmpfolder
    just count_folder "tmpfolder/alacritty" 2

    touch tmpfolder/manfile 
    echo "nonsense" >tmpfolder/.shellrc
    echo "nonsense" >tmpfolder/alacritty/theme.toml
    {{test_prefix}} sync -SY testing/bigtest2.toml tmpfolder
    just output_contains "cat tmpfolder/.shellrc" "nonsense"
    just output_contains "cat tmpfolder/alacritty/theme.toml" "#282a36"
    {{test_prefix}} sync -SY testing/bigtest2.toml tmpfolder --tags="pink"
    just output_contains "cat tmpfolder/.shellrc" "pink"
    just output_contains "cat tmpfolder/alacritty/theme.toml" "#f699cd"

# Here, we test for two things. 
# 1. If the git ids like HEAD or branch^ work correctly
# 2. If a hashed file can be taken from a folder even when its source has become invalid
@bigtest3:test_clean
    just make_test_repo

    # Check that the config syncs with its tag active
    {{test_prefix}} sync -Y testing/bigtest3.toml tmpfolder -t head
    just output_contains "cat tmpfolder/file1_before tmpfolder/file1_now tmpfolder/file1_head" "something\nchanged\nstart over"

    # We delete the repo and sync again without a tag. All untagged files are hashed.
    rm -rf testing/testrepo 
    {{test_prefix}} sync -Y testing/bigtest3.toml tmpfolder 

    # It should fail when the unhashed file is activated
    just error_contains "{{test_prefix}} sync -Y testing/bigtest3.toml tmpfolder -t head" "No valid source in list"

    # When we change a file it should stop working for the hashed files too
    echo nonsense>tmpfolder/file1_now 
    just error_contains "{{test_prefix}} sync -Y testing/bigtest3.toml tmpfolder" "No valid source in list"

    # If we change it back, it works again
    echo changed>tmpfolder/file1_now 
    {{test_prefix}} sync -Y testing/bigtest3.toml tmpfolder

    # We build the repo again, but this time we load a config from it
    just make_test_repo
    {{test_prefix}} sync -Y {{justfile_directory()}}/testing/testrepo#develop^:included3.toml tmpfolder
    just output_contains "cat tmpfolder/file1" "something"
    {{test_prefix}} sync -Y {{justfile_directory()}}/testing/testrepo#develop:included3.toml tmpfolder
    just output_contains "cat tmpfolder/file1" "changed"
    {{test_prefix}} sync -Y {{justfile_directory()}}/testing/testrepo#HEAD:included3.toml tmpfolder
    just output_contains "cat tmpfolder/file1" "start over"

# Creates a repo for testing with two commits on the develop branch.
make_test_repo:
    -rm -rf testing/testrepo
    mkdir testing/testrepo
    cd testing/testrepo && git init
    cd testing/testrepo && git checkout -b develop
    cd testing/testrepo && echo "something" >file1
    cp testing/included3.toml testing/testrepo/
    cd testing/testrepo && git add .
    cd testing/testrepo && git commit -m "first commit"
    cd testing/testrepo && echo "changed" >file1
    cd testing/testrepo && git add .
    cd testing/testrepo && git commit -m "second commit"
    cd testing/testrepo && git checkout -b feature
    cd testing/testrepo && echo "start over" >file1
    cd testing/testrepo && git add .
    cd testing/testrepo && git commit -m "third commit"

# This tests various files that have something wrong with them.
@failure_tests: test_clean 
    # Relative paths are not allowed
    just error_contains "{{test_prefix}} sync testing/failure1.toml tmpfolder/" "relative path"

    # This file works without inclusions. But when the tag is active, to tagges paths collide.
    {{test_prefix}} sync testing/failure2.toml tmpfolder/ -Y
    just error_contains "{{test_prefix}} sync testing/failure2.toml tmpfolder/ -t inc" "two files for path included/main.rs"

    # Here, a hash does not match.
    just error_contains "{{test_prefix}} sync testing/failure3.toml tmpfolder/" "Hash of loaded config"

    # The "count" of a folder does not match its actual contents.
    just error_contains "{{test_prefix}} sync testing/failure4.toml tmpfolder/" "Expected 5 files"

# This config makes some edits to a file. 
# The test is here to ensure that the edits always produce the same output.
@edits_test: test_clean
    {{test_prefix}} sync testing/edits_test.toml tmpfolder --no-confirm
    just check_hash tmpfolder/rustlings_readme.md F0BC491EBBCA0BA3DF0F6E11CB9C2CA97EFAC84BA2A65C8AFADD0D045AD0B4DE
    {{test_prefix}} sync testing/edits_test.toml tmpfolder --no-confirm -t append
    just check_hash tmpfolder/rustlings_readme.md 88C468F15606A5BD5EADA0F0475991A2FC01ACA8032BBC5A254CC74D6AA1274A

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

output_contains command msg:
    #!/usr/bin/env python3
    import subprocess
    result = subprocess.run("{{command}}", shell=True, capture_output=True, text=True)
    if (result.returncode!=0):
        print("Command failed: {{command}}")
        exit(1)
    if ("{{msg}}" not in result.stdout):
        print("Wrong output:")
        print(result.stdout)
        exit(1)

check_hash file hash:
     just output_contains "{{test_prefix}} hash {{file}}" {{hash}}
           
