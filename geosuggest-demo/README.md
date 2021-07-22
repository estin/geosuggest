## geosuggest demo

This is a demonstrating how to use [geosuggest](https://github.com/estin/geosuggest).

[Live demo](https://geosuggest.herokuapp.com/) on [Heroku](https://heroku.com) free quota:
- Please be patient, it will take some time for the app to wake up

In Dockerfile:
 - download and compile [geosuggest](https://github.com/estin/geosuggest) backend
 - build index on [geonames free data](http://download.geonames.org/export/dump/)
 - build [yew](https://github.com/yewstack/yew) based frontend


For local build&start
```bash
$ docker build \
    --build-arg PORT="8000" \
    --build-arg GEOSUGGEST_BASE_API_URL="http://127.0.0.1:8000" \
    --build-arg GEOSUGGEST_RELEASE="master" \
    -t geosuggest-demo .
$ docker run --rm -e PORT=8000 -e RUST_LOG=geosuggest=info -p 8000:8000 -it geosuggest-demo
```
