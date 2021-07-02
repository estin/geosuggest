## geosuggest demo

This is a demonstrating how to [geosuggest](https://github.com/estin/geosuggest) and deploy to [Heroku](https://heroku.com).

[Live demo](https://geosuggest.herokuapp.com/) on [Heroku](https://heroku.com) free quota:
- Please be patient, it will take some time for the app to wake up
- scheduler would work only on running app (the default interval is 1 minute)

In Dockerfile:
 - download and compile [geosuggest](https://github.com/estin/cywad) backend
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
