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
 - simple REST http [api](https://geosuggest.etatarkin.ru/swagger)
 - no external services used

### Based on:
 - [strsim](https://crates.io/crates/strsim)
 - [kiddo](https://crates.io/crates/kiddo)
 - [geoip2](https://crates.io/crates/geoip2)
 - [ntex](https://crates.io/crates/ntex)


## Library

Crate usage [example](https://github.com/estin/geosuggest/tree/master/examples/src/simple.rs)

```bash
$ cargo run -p examples --release --bin simple
```


## Service

Currently from sources only.

```bash
$ git clone https://github.com/estin/geosuggest.git
$ cd geosuggest
$ cargo build --release
```

Build index file

```bash
$ cargo run -p geosuggest-utils --bin geosuggest-build-index --release --features=cli -- \
    from-urls \
    --languages=ru,uk,be,zh,ja \
    --output=/tmp/geosuggest-index.bincode
```

Run

```bash
$ GEOSUGGEST__INDEX_FILE=/tmp/geosuggest-index.bincode \
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

## Test

```bash
$ cargo test --workspace --all-features
```

## License

This project is licensed under

* MIT license ([LICENSE](LICENSE) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
