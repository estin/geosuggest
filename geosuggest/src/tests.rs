use geosuggest_core::Engine;
use ntex::web::{test, App, Error, ServiceConfig};
use ntex::Service;
use ntex::{http, web};

use std::sync::Arc;

fn app_config(cfg: &mut ServiceConfig) {
    let mut engine = Engine::new_from_files(
        "../geosuggest-core/tests/misc/cities-ru.txt",
        Some("../geosuggest-core/tests/misc/names.txt"),
        vec!["ru"],
    )
    .unwrap();

    #[cfg(feature = "geoip2_support")]
    engine
        .load_geoip2("../geosuggest-core/tests/misc/GeoLite2-City-Test.mmdb")
        .unwrap();

    let engine = Arc::new(engine);
    cfg.data(engine).service((
        web::resource("/suggest").to(super::suggest),
        web::resource("/reverse").to(super::reverse),
        #[cfg(feature = "geoip2_support")]
        web::resource("/geoip2").to(super::geoip2),
    ));
}

#[ntex::test]
async fn api_suggest() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/suggest?pattern=Voronezh")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(items[0].get("name").unwrap().as_str().unwrap(), "Voronezh");

    Ok(())
}

#[ntex::test]
async fn api_suggest_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/suggest?pattern=Voronezh&lang=ru&limit=1")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(items[0].get("name").unwrap().as_str().unwrap(), "Воронеж");

    Ok(())
}

#[ntex::test]
async fn api_reverse() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/reverse?lat=51.6372&lng=39.1937&limit=1")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]
            .get("city")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Voronezh"
    );

    Ok(())
}

#[ntex::test]
async fn api_reverse_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/reverse?lat=51.6372&lng=39.1937&lang=ru&limit=1")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]
            .get("city")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Воронеж"
    );

    Ok(())
}

#[cfg(feature = "geoip2_support")]
#[ntex::test]
async fn api_geoip2_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/geoip2?ip=81.2.69.142&lang=ru")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city").unwrap().as_object().unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "Лондон");

    Ok(())
}
