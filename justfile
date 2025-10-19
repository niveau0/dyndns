set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

_default:
    just --list

build:
    cargo build --release

run:
    cargo run --release

test:
    cargo audit
    cargo clippy --release -- -D warnings
    cargo fmt -- --check
    cargo test --release -- --nocapture
