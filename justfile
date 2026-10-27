mod gws_cache 'crates/gws-cache/justfile'

run *args:
    cargo run --package gws-cache-cli --locked --bin gws-cache -- {{ args }}

run-ai *args:
    AGENT=1 cargo run --package gws-cache-cli --locked --bin gws-cache -- {{ args }}

build:
    cargo build --package gws-cache-cli --locked --release --bin gws-cache
    mkdir -p bin
    cp target/release/gws-cache bin/gws-cache

[private]
_test:
    cargo test --package gws-cache-cli --locked --all-targets

[private]
_lint:
    cargo fmt --package gws-cache-cli -- --check
    cargo clippy --package gws-cache-cli --locked --all-targets -- -D warnings

coverage:
    cargo llvm-cov --package gws-cache-cli --exclude-from-report gws-cache --locked --all-targets --fail-under-lines 90

doc:
    RUSTDOCFLAGS='-D warnings' cargo doc --package gws-cache-cli --locked --no-deps

check: _lint _test coverage doc
    just gws_cache check

[private]
_fmt:
    cargo fmt --package gws-cache-cli -- --check

test: _test
    just gws_cache test

lint: _lint
    just gws_cache lint

fmt: _fmt
    just gws_cache fmt-check
