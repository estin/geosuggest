## geosuggest demo

This is a demonstrating how to use [geosuggest](https://github.com/estin/geosuggest).

[Live demo](https://geosuggest.etatarkin.ru/)

In Dockerfile:
 - download and compile [geosuggest](https://github.com/estin/geosuggest) backend
 - build index on [geonames free data](http://download.geonames.org/export/dump/)
 - build [sycamore](https://github.com/sycamore-rs/sycamore) based frontend

For local build&start
```bash
$ docker build \
    --build-arg PORT="8000" \
    --build-arg GEOSUGGEST_BASE_API_URL="http://127.0.0.1:8000" \
    --build-arg GEOSUGGEST_RELEASE="master" \
    -t geosuggest-demo .
$ docker run --rm -e PORT=8000 -e RUST_LOG=geosuggest=info -p 8000:8000 -it geosuggest-demo
```
