default:
    just check

check:
    cargo check --workspace --locked

test:
    cargo test --workspace --locked

clippy:
    cargo clippy --workspace --all-targets --locked -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

ci:
    just fmt-check
    just check
    just test
    just clippy
