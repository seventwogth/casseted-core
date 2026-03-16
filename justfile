default:
    just check

check:
    cargo check --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt --all
