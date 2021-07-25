<div align="center">
 <p><h1>geosuggest</h1> </p>
  <p><strong>Library/Service to suggest and to find nearest by coordinates cities</strong></p>
  <p></p>
</div>

[Live demo](https://geosuggest.herokuapp.com/) with [sources](https://github.com/estin/geosuggest/tree/master/geosuggest-demo)

Main features:
 - library or service modes
 - build index by free gazetteer data from [geonames.org](https://www.geonames.org/)
 - suggest city by name
 - find nearest city by coordinates
 - multilang (based on configured index options)
 - simple REST http api
 - no extral api used

Based on:
 - [strsim](https://crates.io/crates/strsim)
 - [kdtree](https://crates.io/crates/kdtree)

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
$ curl -sL http://download.geonames.org/export/dump/cities15000.zip --output /tmp/cities1500.zip \
    && curl -sL http://download.geonames.org/export/dump/alternateNamesV2.zip --output /tmp/alternateNamesV2.zip \
    && unzip -d /tmp /tmp/cities1500.zip \
    && unzip -d /tmp /tmp/alternateNamesV2.zip

# build index
$ cargo run -p geosuggest-utils --bin geosuggest-build-index --release -- -c /tmp/cities15000.txt -n /tmp/alternateNamesV2.txt -l ru,uk,be,zh,ja -o /tmp/geosuggest-index.json
```

Run

```bash
$ RUST_LOG=geosuggest=trace GEOSUGGEST_INDEX_FILE=/tmp/geosuggest-index.json GEOSUGGEST_HOST=127.0.0.1 GEOSUGGEST_PORT=8080 cargo run -p geosuggest --bin geosuggest --release
```

Check

```bash
$ curl "http://127.0.0.1:8080/api/city/suggest?pattern=Voronezh&limit=1"
{"items":[{"id":472045,"name":"Voronezh","country_code":"RU","timezone":"Europe/Moscow","latitude":51.67204,"longitude":39.1843,"population":848752}],"time":15
```

See also demo [Dockerfile](https://github.com/estin/geosuggest/blob/master/geosuggest-demo/Dockerfile)

## License

This project is licensed under

* MIT license ([LICENSE](LICENSE) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
