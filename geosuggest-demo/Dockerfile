FROM rust:1.84-slim-bullseye

ARG GEOSUGGEST_BASE_API_URL=$GEOSUGGEST_BASE_API_URL
ARG GEOSUGGEST_RELEASE=$GEOSUGGEST_RELEASE
ARG GEOSUGGEST_INDEX=geosuggest-index.bincode
ARG CITIES=cities5000

RUN apt-get update \
    && apt-get -y install curl build-essential unzip pkg-config libssl-dev

# geosuggest
RUN cd /root \
    && curl -sL https://github.com/estin/geosuggest/archive/$GEOSUGGEST_RELEASE.zip --output geosuggest-release.zip \
    && unzip geosuggest-release.zip \
    && mv geosuggest-$GEOSUGGEST_RELEASE geosuggest-release \
    && cd geosuggest-release \
    && cargo build --release --features=tokio,cli

# index
RUN cd /tmp \ 
    && curl -sL http://download.geonames.org/export/dump/$CITIES.zip --output $CITIES.zip \
    && curl -sL http://download.geonames.org/export/dump/alternateNamesV2.zip --output alternateNamesV2.zip \
    && curl -sL http://download.geonames.org/export/dump/admin1CodesASCII.txt --output /tmp/admin1CodesASCII.txt \
    && curl -sL http://download.geonames.org/export/dump/admin2Codes.txt --output /tmp/admin2Codes.txt \
    && curl -sl http://download.geonames.org/export/dump/countryInfo.txt --output /tmp/countryInfo.txt \
    && unzip $CITIES.zip \
    && unzip alternateNamesV2.zip

# fix new Russian regions
RUN sed -ie "s/UA\t\t11/RU\t\t192/p" /tmp/$CITIES.txt
RUN sed -ie "s/UA\t\t20/RU\t\t193/p" /tmp/$CITIES.txt
RUN sed -ie "s/UA\t\t05/RU\t\t194/p" /tmp/$CITIES.txt
RUN sed -ie "s/UA\t\t14/RU\t\t195/p" /tmp/$CITIES.txt
RUN sed -ie "s/UA\t\t08/RU\t\t196/p" /tmp/$CITIES.txt
RUN sed -ie "s/UA\t\t26/RU\t\t197/p" /tmp/$CITIES.txt

RUN ls /tmp
RUN /root/geosuggest-release/target/release/geosuggest-build-index from-files \
    --cities=/tmp/$CITIES.txt \
    --names=/tmp/alternateNamesV2.txt \
    --admin-codes=/tmp/admin1CodesASCII.txt \
    --admin2-codes=/tmp/admin2Codes.txt \
    --countries=/tmp/countryInfo.txt \
    --languages=ru,uk,be,zh,ja \
    --output=/opt/$GEOSUGGEST_INDEX

# demo ui
RUN cargo install wasm-bindgen-cli
RUN cargo install trunk
RUN rustup target add wasm32-unknown-unknown
RUN cd /root/geosuggest-release/geosuggest-demo \
    && trunk build --release -d /root/geosuggest-demo-release

# final stage
FROM debian:bookworm-slim

ARG PORT=$PORT
ARG GEOSUGGEST_INDEX=geosuggest-index.bincode

COPY --from=0 /root/geosuggest-release/target/release/geosuggest /usr/bin
COPY --from=0 /root/geosuggest-demo-release /opt/geosuggest-static
COPY --from=0 /opt/$GEOSUGGEST_INDEX /opt/$GEOSUGGEST_INDEX

# default configuration
EXPOSE $PORT
ENV GEOSUGGEST_CONFIG_FILE="/opt/geosuggest-settings.toml"
ENV RUST_LOG=geosuggest=trace

RUN echo "host = '0.0.0.0'" >> /opt/geosuggest-settings.toml \
    && echo "port = 8000 # see GEOSUGGEST_PORT" >> /opt/geosuggest-settings.toml \
    && echo "index_file = '/opt/$GEOSUGGEST_INDEX'" >> /opt/geosuggest-settings.toml \
    && echo "static_dir = '/opt/geosuggest-static'" >> /opt/geosuggest-settings.toml \
    && echo "url_path_prefix = '/'" >> /opt/geosuggest-settings.toml

# pass $PORT to $GEOSUGGEST_PORT
ENTRYPOINT []
CMD GEOSUGGEST__PORT=$PORT geosuggest
