test_prefix:="cargo run"
exists:="test -e"
absent:="test ! -e"

clean:
    -rm -r target
    -rm -r tmpfolder 

test_clean:
    -rm -r tmpfolder 

fmt:
    cargo fmt

test: fmt
    cargo test
    just test1 test2

build: test 
    cargo build --release 
    cargo build --release --target=x86_64-unknown-linux-musl --features=static


test1:test_clean
    {{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm
    {{exists}} tmpfolder/description.txt|echo
    {{exists}} tmpfolder/rustlings_readme.md
    just count_folder tmpfolder 3
    just count_folder tmpfolder/subfolder 2
    {{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm -t file2
    just test_fails "{{test_prefix}} sync testing/testconfig1.toml tmpfolder --no-confirm -t wrongtag"
    just count_folder tmpfolder 3
    just count_folder tmpfolder/subfolder 3

test2:test_clean
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm
    just count_folder tmpfolder 2
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t subfolder
    just count_folder tmpfolder 3
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm
    just count_folder tmpfolder 2
    {{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t conflict
    just count_folder tmpfolder 3
    just test_fails "{{test_prefix}} sync testing/testconfig2.toml tmpfolder --no-confirm -t conflict -t subfolder"
    {{absent}} tmpfolder/shouldnotexist

count_folder folder expected:
    #!/usr/bin/env python3
    import os
    assert(len(os.listdir("{{folder}}"))=={{expected}})
test_fails command:
    #!/usr/bin/env python3
    import os
    assert(os.system("{{command}}")!=0)


