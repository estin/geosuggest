<div align="center">
  <p><h1>geosuggest</h1> </p>
  <p><strong>Library/Service to suggest and to find nearest by coordinates cities</strong></p>
  <p></p>
</div>

[Live demo](https://geosuggest.etatarkin.ru/) with [sources](https://github.com/estin/geosuggest/tree/master/geosuggest-demo)

Main features:
 - library or service modes
 - build index by free gazetteer data from [geonames.org](https://www.geonames.org/)
 - suggest city by name
 - find nearest city by coordinates
 - MaxMind GeoIP2(Lite) city database support
 - multilang (based on configured index options)
 - simple REST http [api](https://geosuggest.herokuapp.com/swagger)
 - no external services used

### Based on:
 - [strsim](https://crates.io/crates/strsim)
 - [kiddo](https://crates.io/crates/kiddo)
 - [geoip2](https://crates.io/crates/geoip2)
 - [ntex](https://crates.io/crates/ntex)

## Setup&Run

Currently from sources only.

```bash
$ git clone https://github.com/estin/geosuggest.git
$ cd geosuggest
$ cargo build --release
```

Build index file

```bash
# download raw data from geonames
$ curl -sL http://download.geonames.org/export/dump/cities15000.zip --output /tmp/cities15000.zip \
    && curl -sL http://download.geonames.org/export/dump/alternateNamesV2.zip --output /tmp/alternateNamesV2.zip \
    && curl -sL http://download.geonames.org/export/dump/admin1CodesASCII.txt --output /tmp/admin1CodesASCII.txt \
    && unzip -d /tmp /tmp/cities15000.zip \
    && unzip -d /tmp /tmp/alternateNamesV2.zip

# build index
$ cargo run -p geosuggest-utils --bin geosuggest-build-index --release -- \
    -c /tmp/cities15000.txt \
    -n /tmp/alternateNamesV2.txt \
    -a /tmp/admin1CodesASCII.txt \
    -l ru,uk,be,zh,ja \
    --countries geosuggest-core/tests/misc/country-info.txt \
    -o /tmp/geosuggest-index.bincode
```

Run

```bash
$ RUST_LOG=geosuggest=trace \
    GEOSUGGEST__INDEX_FILE=/tmp/geosuggest-index.bincode \
    GEOSUGGEST__HOST=127.0.0.1 \
    GEOSUGGEST__PORT=8080 \
    GEOSUGGEST__URL_PATH_PREFIX="/" \
    cargo run -p geosuggest --bin geosuggest --release
```

Check

```bash
$ curl -s "http://127.0.0.1:8080/api/city/suggest?pattern=Voronezh&limit=1" | jq
{
  "items": [
    {
      "id": 472045,
      "name": "Voronezh",
      "country": {
        "id": 2017370,
        "code": "RU",
        "name": "Russia"
      },
      "admin_division": {
        "id": 472039,
        "code": "RU.86",
        "name": "Voronezj"
      },
      "timezone": "Europe/Moscow",
      "latitude": 51.67204,
      "longitude": 39.1843,
      "population": 848752
    }
  ],
  "time": 24
}
```

See also demo [Dockerfile](https://github.com/estin/geosuggest/blob/master/geosuggest-demo/Dockerfile)

Test

```bash
$ cargo test --all-features -- --test-threads=1
```

## License

This project is licensed under

* MIT license ([LICENSE](LICENSE) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
