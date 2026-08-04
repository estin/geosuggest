list:
    just --list

clippy:
    cargo clippy --workspace --fix --broken-code --allow-dirty --allow-staged --no-default-features --features="tokio,geoip2,tracing"

test:
    cargo nextest run --no-default-features --features="tokio,geoip2,tracing"

example:
    cargo run -p geosuggest-examples --release --bin simple

ci: clippy test example

release:
    cargo publish -p geosuggest-core --all-features
    cargo publish -p geosuggest-utils --all-features
