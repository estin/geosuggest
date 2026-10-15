list:
    just --list

clippy:
    cargo clippy --workspace --fix --broken-code --allow-dirty --allow-staged --no-default-features --features="tokio,geoip2,tracing"

lint:
    cargo clippy --workspace --no-default-features --features="tokio,geoip2,tracing" -- -D warnings

test:
    cargo nextest run --workspace --no-default-features --features="tokio,geoip2,tracing"
    cargo nextest run -p geosuggest --no-default-features --features="tokio,geoip2,tracing,demo"
    cargo test --doc --workspace --no-default-features --features="tokio,geoip2,tracing"

example:
    cargo run -p geosuggest-examples --release --bin simple

run:
    #!/usr/bin/env bash
    set -euo pipefail
    index_file="/tmp/geosuggest-index.rkyv"
    server_url="http://127.0.0.1:8080/swagger"

    command -v curl >/dev/null 2>&1 || { echo "curl is required to check whether the server is running" >&2; exit 1; }
    if curl --silent --fail --max-time 2 "$server_url" >/dev/null; then
        page="$(curl --silent --fail --max-time 2 http://127.0.0.1:8080/ || true)"
        if [[ "$page" == *"<title>GEOSUGGEST</title>"* && "$page" == *'id="city-query"'* ]]; then
            echo "Geosuggest demo and Swagger are already running on port 8080"
            exit 0
        fi
        echo "A server already uses port 8080 but does not serve the demo. Stop it, then run just run again." >&2
        exit 1
    fi
    if curl --silent --output /dev/null --max-time 2 http://127.0.0.1:8080/api/city/get?id=0; then
        echo "A server already responds on port 8080 but does not serve the demo and Swagger. Stop it, then run just run again." >&2
        exit 1
    fi

    if [[ -s "$index_file" ]]; then
        echo "Using existing index: $index_file"
    else
        cargo run -p geosuggest-utils --bin geosuggest-build-index --release --features=cli,tracing -- from-urls --languages=ru,uk,be,zh,ja --output="$index_file"
    fi

    if ! command -v cargo-topcoat >/dev/null 2>&1; then
        cargo install topcoat-cli --locked
    fi

    cargo-topcoat asset bundle --release --package geosuggest-demo --bin geosuggest-demo-assets

    GEOSUGGEST__INDEX_FILE="$index_file" GEOSUGGEST__HOST=127.0.0.1 GEOSUGGEST__PORT=8080 GEOSUGGEST__URL_PATH_PREFIX="/" cargo run --release --bin geosuggest --features=demo

rebuild-index:
    cargo run -p geosuggest-utils --bin geosuggest-build-index --release --features=cli,tracing -- from-urls --languages=ru,uk,be,zh,ja --output=/tmp/geosuggest-index.rkyv

ci: lint test example

bump:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -n "$(git tag --list 'v[0-9]*')" ]] || { echo "no version tags found; fetch tags first" >&2; exit 1; }
    VERSION="$(git-cliff --bumped-version)"
    VERSION="${VERSION#v}"
    [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "git-cliff returned invalid version: $VERSION" >&2; exit 1; }
    SERIES="${VERSION%.*}"
    git-cliff --tag "v$VERSION" --output CHANGELOG.md
    sed -i -E '/^\[workspace\.package\]/,/^\[/ s/^version = .*/version = "'"$VERSION"'"/' Cargo.toml
    sed -i -E 's/(geosuggest-(core|utils) = \{ path = "[^"]+", version = ")[^"]+/\1'"$SERIES"'/' geosuggest*/Cargo.toml
    echo "Updated workspace version to $VERSION"

release:
    cargo publish -p geosuggest-core --all-features
    cargo publish -p geosuggest-utils --all-features
