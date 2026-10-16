## Geosuggest demo

The geosuggest server renders this demo with Topcoat. Search results use the
loaded index directly. The demo does not call the JSON API.

Run `just run` from the repository root to build an index when needed, bundle
the Topcoat runtime, and start the server. Open `http://127.0.0.1:8080/` for
the demo and `http://127.0.0.1:8080/swagger` for the API page.
