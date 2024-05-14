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
    just bigtest1 bigtest2 gittest failure_tests edits_test 

build: test 
    cargo build --release

install:
    cargo install --path="{{justfile_directory()}}"

install_debug:
    cargo install --path="{{justfile_directory()}}" --features=debug

# This should result in a statically linked binary. Sometimes musl builds fail the first time
build_musl: test
    -cargo build --release --target=x86_64-unknown-linux-musl 
    cargo build --release --target=x86_64-unknown-linux-musl 
    just output_contains "ldd target/x86_64-unknown-linux-musl/release/lorevault" "statically linked"
    
@bigtest1:test_clean
    {{test_prefix}} sync testing/bigtest1.toml tmpfolder --no-confirm
    just count_folder "tmpfolder" 3
    just error_contains "diff src/main.rs tmpfolder/program_src/main.rs" ""
    just output_contains "{{test_prefix}} tags testing/bigtest1.toml" "replace_main"
    {{test_prefix}} sync testing/bigtest1.toml tmpfolder --no-confirm -t replace_main
    just output_contains "diff src/main.rs tmpfolder/program_src/main.rs" ""
    just output_contains "grep Freddy tmpfolder/Dracula.txt | wc -l" "41" 
    {{test_prefix}} sync testing/bigtest1.toml tmpfolder --no-confirm --tags=replace_main,inc 
    just count_folder "tmpfolder" 4
    just output_contains "diff testing/testfolder tmpfolder/included" ""

@bigtest2:test_clean
    {{test_prefix}} sync -Y testing/bigtest2.toml tmpfolder 
    just count_folder "tmpfolder" 1
    touch tmpfolder/manfile 
    just count_folder "tmpfolder" 2
    {{test_prefix}} sync -SY testing/bigtest2.toml tmpfolder
    just count_folder "tmpfolder" 2
    {{test_prefix}} sync -Y testing/bigtest2.toml tmpfolder
    just count_folder "tmpfolder" 1
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

@gittest:test_clean
    mkdir testing/testrepo
    cd testing/testrepo && git init
    cd testing/testrepo && git checkout -b develop
    cd testing/testrepo && echo "something" >file1
    cd testing/testrepo && git add file1
    cd testing/testrepo && git commit -m "first commit"
    cd testing/testrepo && echo "changed" >file1
    cd testing/testrepo && git add file1
    cd testing/testrepo && git commit -m "second commit"
    {{test_prefix}} sync -Y testing/gittest.toml tmpfolder 
    just output_contains "cat tmpfolder/file1_before tmpfolder/file1_now" "something\nchanged"












    
@failure_tests: test_clean 
    just error_contains "{{test_prefix}} sync testing/failure1.toml tmpfolder/" "relative path"
    just error_contains "{{test_prefix}} sync testing/failure2.toml tmpfolder/ -t inc" "two files for path included/main.rs"
    just error_contains "{{test_prefix}} sync testing/failure3.toml tmpfolder/" "Hash of loaded config"
    just error_contains "{{test_prefix}} sync testing/failure4.toml tmpfolder/" "Expected 5 files"

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
           
