lint:
    pre-commit run --all-files

test:
    cargo test

install:
    cargo install --path .

update:
    cargo update
