use std::boxed::Box;

#[cfg(feature = "demo")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "geoip2")]
use std::net::SocketAddr;

use bonfig::FromSource as _;
use geosuggest_core::{index::SourceFileOptions, EngineData};
use topcoat::router::{
    header, request::Request, response::Response, to_bytes, Body, Router, StatusCode,
};

#[cfg(feature = "geoip2")]
use topcoat::router::RemoteAddr;

#[cfg(feature = "swagger")]
use super::generate_openapi_files;
use super::{build_router_with_assets, settings::Settings, AppState, DemoAssetConfig};

fn build_test_router(settings: Settings) -> Router {
    #[cfg(feature = "demo")]
    let assets = None;
    #[cfg(not(feature = "demo"))]
    let assets = ();
    build_test_router_with_assets(settings, assets)
}

fn build_test_router_with_assets(settings: Settings, assets: DemoAssetConfig) -> Router {
    let data = geosuggest_core::index::IndexData::new_from_files(SourceFileOptions {
        cities: "../geosuggest-core/tests/misc/cities.txt",
        names: Some("../geosuggest-core/tests/misc/names.txt"),
        countries: Some("../geosuggest-core/tests/misc/country-info.txt"),
        filter_languages: vec!["ru"],
        admin1_codes: Some("../geosuggest-core/tests/misc/admin1-codes.txt"),
        admin2_codes: Some("../geosuggest-core/tests/misc/admin2-codes.txt"),
        excluded_feature_codes: geosuggest_core::index::DEFAULT_EXCLUDED_FEATURE_CODES.to_vec(),
    })
    .unwrap();

    let engine_data = EngineData::try_from(data).unwrap();
    #[cfg(feature = "geoip2")]
    let mut engine_data = engine_data;
    #[cfg(feature = "geoip2")]
    engine_data
        .load_geoip2("../geosuggest-core/tests/misc/GeoLite2-City-Test.mmdb")
        .unwrap();

    let engine_data = Box::leak(Box::new(engine_data));
    let engine = Box::leak(Box::new(
        engine_data.as_engine().expect("test engine initializes"),
    ));
    let app_state = AppState {
        engine,
        settings: settings.clone(),
    };

    build_router_with_assets(app_state, &settings, assets)
}

fn settings(prefix: &str, static_dir: Option<&str>) -> Settings {
    let mut source = format!("url_path_prefix = {prefix:?}");
    if let Some(static_dir) = static_dir {
        source.push_str(&format!("\nstatic_dir = {static_dir:?}"));
    }
    Settings::from_toml(&source).unwrap()
}

fn request(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

async fn response_body(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn api_routes_use_the_configured_prefix() {
    let app = build_test_router(settings("/geosuggest", None));

    let response = app
        .handle(request("/geosuggest/api/city/get?id=472045"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["city"]["name"], "Voronezh");

    let response = app.handle(request("/api/city/get?id=472045")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn capital_suggest_and_reverse_routes_keep_their_results() {
    let app = build_test_router(settings("/", None));

    let response = app
        .handle(request("/api/city/capital?country_code=RU"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["city"]["name"], "Moscow");

    let response = app
        .handle(request(
            "/api/city/capital?lat=55.7558&lng=37.6173&country_code=GB",
        ))
        .await;
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["city"]["name"], "Moscow");

    let response = app
        .handle(request(
            "/api/city/suggest?pattern=Voronezh&countries=RU,JP&lang=ru&limit=1",
        ))
        .await;
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["items"][0]["name"], "Воронеж");

    let response = app
        .handle(request(
            "/api/city/reverse?lat=51.6372&lng=39.1937&limit=1&lang=ru",
        ))
        .await;
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["items"][0]["city"]["name"], "Воронеж");
}

#[cfg(feature = "swagger")]
#[tokio::test]
async fn docs_routes_serve_openapi_swagger_and_redoc() {
    let settings = settings("/geosuggest", None);
    generate_openapi_files(&settings).unwrap();
    let app = build_test_router(settings);

    let response = app.handle(request("/geosuggest/openapi3.yaml")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(String::from_utf8(response_body(response).await)
        .unwrap()
        .contains("/geosuggest"));

    for path in ["/geosuggest/swagger", "/geosuggest/redoc"] {
        let response = app.handle(request(path)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
    }
}

#[cfg(feature = "demo")]
#[tokio::test]
async fn root_serves_static_html_with_an_html_content_type() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let static_dir = std::env::temp_dir().join(format!("geosuggest-static-{unique}"));
    std::fs::create_dir_all(&static_dir).unwrap();
    std::fs::write(static_dir.join("index.html"), "<html>static index</html>").unwrap();

    let app = build_test_router(settings("/geosuggest", Some(static_dir.to_str().unwrap())));
    let response = app.handle(request("/geosuggest")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(response_body(response).await, b"<html>static index</html>");

    let response = app
        .handle(request("/geosuggest/api/city/get?id=472045"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    std::fs::remove_dir_all(static_dir).unwrap();
}

#[cfg(feature = "demo")]
#[tokio::test]
async fn demo_page_renders_topcoat_shards_on_the_server() {
    let assets = geosuggest_demo::bundle_assets()
        .into_iter()
        .enumerate()
        .map(|(index, asset)| topcoat::asset::ManifestEntry {
            id: asset.id(),
            file: format!("asset-{index}.bin"),
            hash: format!("test-{index}"),
            content_type: "application/octet-stream".to_owned(),
        })
        .collect();
    let assets = topcoat::asset::AssetConfig::hosted_at(
        "/test-assets",
        topcoat::asset::Manifest { version: 1, assets },
    );
    let app = build_test_router_with_assets(settings("/", None), Some(assets));
    let response = app.handle(request("/")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );

    let html = String::from_utf8(response_body(response).await).unwrap();
    assert!(html.contains("<title>GEOSUGGEST</title>"));
    assert!(html.contains("Search results will appear here."));
    assert!(html.contains("Nearby cities will appear here."));
    assert!(html.contains("Select a city result to view its details."));
    assert!(html.contains("::topcoat::shard::start(\"/_geosuggest/suggest\""));
    assert!(html.contains("::topcoat::shard::start(\"/_geosuggest/reverse\""));
    assert!(html.contains("::topcoat::shard::start(\"/_geosuggest/city\""));
    assert!(html.contains("/test-assets/"));
    assert!(!html.contains("/api/city/"));
}

#[cfg(feature = "demo")]
#[tokio::test]
async fn demo_page_is_not_served_under_a_prefix() {
    let app = build_test_router(settings("/geosuggest", None));

    for path in ["/", "/geosuggest"] {
        let response = app.handle(request(path)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let response = app
        .handle(request("/geosuggest/api/city/get?id=472045"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[cfg(not(any(feature = "demo", feature = "swagger")))]
#[tokio::test]
async fn api_only_build_does_not_register_ui_routes() {
    let app = build_test_router(settings("/", None));

    for path in ["/", "/swagger", "/redoc", "/openapi3.yaml"] {
        let response = app.handle(request(path)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn cors_allows_browser_get_and_preflight_requests() {
    let app = build_test_router(settings("/", None));

    let response = app
        .handle(
            Request::builder()
                .uri("/api/city/get?id=472045")
                .header(header::ORIGIN, "https://client.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "https://client.example"
    );

    let response = app
        .handle(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/city/get")
                .header(header::ORIGIN, "https://client.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "x-client")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .unwrap(),
        "GET"
    );
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .unwrap(),
        "x-client"
    );
}

#[cfg(feature = "geoip2")]
#[tokio::test]
async fn geoip_uses_the_peer_address_when_no_ip_is_given() {
    let app = build_test_router(settings("/", None));
    let peer: SocketAddr = "81.2.69.142:4242".parse().unwrap();
    let response = app
        .handle(
            Request::builder()
                .uri("/api/city/geoip2?lang=ru")
                .extension(RemoteAddr(peer))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["for_ip"], "81.2.69.142");
    assert_eq!(result["city"]["name"], "Лондон");

    let response = app
        .handle(
            Request::builder()
                .uri("/api/city/geoip2?ip=client&lang=ru")
                .header(header::FORWARDED, "81.2.69.142")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = serde_json::from_slice(&response_body(response).await).unwrap();
    assert_eq!(result["city"]["name"], "Лондон");
}
