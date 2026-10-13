list:
    just --list

clippy:
    cargo clippy --workspace --fix --broken-code --allow-dirty --allow-staged --no-default-features --features="tokio,geoip2,tracing"

test:
    cargo nextest run --no-default-features --features="tokio,geoip2,tracing"

example:
    cargo run -p geosuggest-examples --release --bin simple

ci: clippy test example

bump:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -n "$(git tag --list 'v[0-9]*')" ]] || { echo "no version tags found; fetch tags first" >&2; exit 1; }
    VERSION="$(git-cliff --bumped-version)"
    VERSION="${VERSION#v}"
    [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "git-cliff returned invalid version: $VERSION" >&2; exit 1; }
    SERIES="${VERSION%.*}"
    sed -i -E '/^\[workspace\.package\]/,/^\[/ s/^version = .*/version = "'"$VERSION"'"/' Cargo.toml
    sed -i -E 's/(geosuggest-(core|utils) = \{ path = "[^"]+", version = ")[^"]+/\1'"$SERIES"'/' geosuggest*/Cargo.toml
    echo "Updated workspace version to $VERSION"

release:
    cargo publish -p geosuggest-core --all-features
    cargo publish -p geosuggest-utils --all-features
