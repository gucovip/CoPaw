# justfile for CoPaw - convenient commands for development

default:
    @cargo run -p copaw

dev:
    @cargo run -p copaw

build:
    @cargo build --release

test:
    @cargo test

check:
    @cargo check --all

fmt:
    @cargo fmt --all

clippy:
    @cargo clippy --all -- -D warnings

run: dev
