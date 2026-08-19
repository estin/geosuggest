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
 - multi-language (based on configured index options)
 - simple REST http [api](https://geosuggest.etatarkin.ru/swagger)
 - no external services used

### Based on:
 - [strsim](https://crates.io/crates/strsim)
 - [kiddo](https://crates.io/crates/kiddo)
 - [geoip2](https://crates.io/crates/geoip2)
 - [rkyv](https://crates.io/crates/rkyv)
 - [ntex](https://crates.io/crates/ntex)


## Library

Crate usage [example](https://github.com/estin/geosuggest/blob/master/geosuggest-examples/src/simple.rs)

```console
$ cargo run -p geosuggest-examples --release --bin simple
```


## Index feature-code filtering

`geosuggest` builds its search index from GeoNames `cities*.txt` dumps, which
classify every record by a `feature_code` (see
[geonames.org/export/codes.html](https://www.geonames.org/export/codes.html)).

To keep the index small, the builder **excludes** a set of `feature_code`s by
default. Exclusion is on by default via `DEFAULT_EXCLUDED_FEATURE_CODES`; the
ability to *customize* the list is opt-in and does not change the default index
size unless you override it.

The default exclusion list (`geosuggest_core::index::DEFAULT_EXCLUDED_FEATURE_CODES`):

```
PPLA3, PPLA4, PPLA5, PPLF, PPLL, PPLQ, PPLW, PPLX, STLMT
```

> **Note:** `--excluded-feature-codes` is a **full replacement** of the default
> list, not an additive filter. Whatever you pass becomes the complete exclusion
> set, so re-list every code you still want excluded. Passing an empty value
> (`--excluded-feature-codes=`) excludes nothing and indexes every record.

To include records that are excluded by default, pass a narrower list. For
example, to also index third-order administrative seats (`PPLA3`) such as
Cartagena or Badalona, re-list the *other* eight default codes you still want
excluded:

```console
$ cargo run -p geosuggest-utils --bin geosuggest-build-index --release --features=cli,tracing -- \
    from-urls \
    --languages=ru,uk,be,zh,ja \
    --excluded-feature-codes=PPLA4,PPLA5,PPLF,PPLL,PPLQ,PPLW,PPLX,STLMT \
    --output=/tmp/geosuggest-index.rkyv
```

To index only capital cities (exclude everything except `PPLC`), combine the
default nine with the broader `PPL*` codes you also want excluded:

```console
--excluded-feature-codes=PPL,PPLA,PPLA2,PPLS,PPLG,PPLA3,PPLA4,PPLA5,PPLF,PPLL,PPLQ,PPLW,PPLX,STLMT
```

The option is available in two places:

| API | Field |
|-----|-------|
| `geosuggest_core::index::SourceFileOptions` / `SourceFileContentOptions` | `excluded_feature_codes: Vec<&str>` |
| `geosuggest_utils::IndexUpdaterSettings` (CLI `--excluded-feature-codes`) | `excluded_feature_codes: Vec<&str>` |

### GeoNames `PPL*` / `STLMT` feature codes

| Code | Meaning | Default |
|------|---------|---------|
| `PPL`  | populated place | included |
| `PPLA` | seat of a first-order administrative division | included |
| `PPLA2`| seat of a second-order administrative division | included |
| `PPLA3`| seat of a third-order administrative division | **excluded** |
| `PPLA4`| seat of a fourth-order administrative division | **excluded** |
| `PPLA5`| seat of a fifth-order administrative division | **excluded** |
| `PPLC` | capital of a political entity | included |
| `PPLCH`| historical capital of a political entity | included |
| `PPLF` | farm village | **excluded** |
| `PPLG` | seat of government of a political entity | included |
| `PPLH` | historical populated place | included |
| `PPLL` | populated locality | **excluded** |
| `PPLQ` | abandoned populated place | **excluded** |
| `PPLR` | religious populated place | included |
| `PPLS` | populated places | included |
| `PPLW` | destroyed populated place | **excluded** |
| `PPLX` | section of populated place | **excluded** |
| `STLMT`| Israeli settlement | **excluded** |

## Service

Install from sources (preferred).

```console
$ git clone https://github.com/estin/geosuggest.git
$ cd geosuggest
$ cargo build --release
```

Build index file

```console
$ cargo run -p geosuggest-utils --bin geosuggest-build-index --release --features=cli,tracing -- \
    from-urls \
    --languages=ru,uk,be,zh,ja \
    --output=/tmp/geosuggest-index.rkyv
```

Run

```console
$ GEOSUGGEST__INDEX_FILE=/tmp/geosuggest-index.rkyv \
    GEOSUGGEST__HOST=127.0.0.1 \
    GEOSUGGEST__PORT=8080 \
    GEOSUGGEST__URL_PATH_PREFIX="/" \
    cargo run -p geosuggest --bin geosuggest --release
```

Check

```console
$ curl -s "http://127.0.0.1:8080/api/city/suggest?pattern=Voronezh&limit=1" | jq
```

```json
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
      "admin2_division": null,
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

```console
$ cargo test --workspace
```

## License

This project is licensed under

* MIT license ([LICENSE](LICENSE) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
