list:
    just --list

clippy:
    cargo clippy --workspace --fix --broken-code --allow-dirty --allow-staged --no-default-features --features="tokio,geoip2,tracing"

test:
    cargo nextest --no-default-features --features="tokio,geoip2,tracing"

release:
    cargo publish -p geosuggest-core --all-features
    cargo publish -p geosuggest-utils --all-features
