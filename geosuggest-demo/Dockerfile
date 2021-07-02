FROM rust:slim

ARG GEOSUGGEST_BASE_API_URL=$GEOSUGGEST_BASE_API_URL
ARG GEOSUGGEST_RELEASE=$GEOSUGGEST_RELEASE
ARG GEOSUGGEST_INDEX=geosuggest-index.json

RUN apt update \
    && apt -y install curl build-essential unzip pkg-config libssl-dev

# geosuggest
RUN cd /root \
    && curl -sL https://github.com/estin/geosuggest/archive/$GEOSUGGEST_RELEASE.zip --output geosuggest-release.zip \
    && unzip geosuggest-release.zip \
    && mv geosuggest-$GEOSUGGEST_RELEASE geosuggest-release \
    && cd geosuggest-release \
    && cargo build --release

# index

RUN cd /tmp \ 
    && curl -sL http://download.geonames.org/export/dump/cities15000.zip --output cities1500.zip \
    && curl -sL http://download.geonames.org/export/dump/alternateNamesV2.zip --output alternateNamesV2.zip \
    && unzip cities1500.zip \
    && unzip alternateNamesV2.zip

RUN ls /tmp
RUN /root/geosuggest-release/target/release/geosuggest-build-index -c /tmp/cities15000.txt -n /tmp/alternateNamesV2.txt -l ru,uk,be,zh,ja -o /opt/$GEOSUGGEST_INDEX

# demo ui
RUN cargo install trunk wasm-bindgen-cli
RUN rustup target add wasm32-unknown-unknown
RUN cd /root/geosuggest-release/geosuggest-demo \
    && trunk build --release -d /root/geosuggest-demo-release

# final stage
FROM debian:buster-slim

ARG PORT=$PORT
ARG GEOSUGGEST_INDEX=geosuggest-index.json

COPY --from=0 /root/geosuggest-release/target/release/geosuggest /usr/bin
# COPY --from=0 /root/geosuggest-ui-release /opt/geosuggest-static
COPY --from=0 /opt/$GEOSUGGEST_INDEX /opt/$GEOSUGGEST_INDEX

# default configuration
RUN echo "host = '0.0.0.0'" >> /opt/geosuggest-settings.toml \
    && echo "port = '$PORT'" >> /opt/geosuggest-settings.toml \
    && echo "index_file = '/opt/$GEOSUGGEST_INDEX'" >> /opt/geosuggest-settings.toml
    && echo "static_dir = '/opt/geosuggest-static'" >> /opt/geosuggest-settings.toml

ENTRYPOINT []
ENV GEOSUGGEST_CONFIG_FILE="/opt/geosuggest-settings.toml"
CMD geosuggest
