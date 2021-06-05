use geosuggest_core::Engine;
use ntex::web::{test, App, Error, ServiceConfig};
use ntex::Service;
use ntex::{http, web};

use std::sync::Arc;

fn app_config(cfg: &mut ServiceConfig) {
    let engine = Arc::new(
        Engine::new_from_files(
            "../geosuggest-core/tests/misc/cities-ru.txt",
            Some("../geosuggest-core/tests/misc/names.txt"),
            vec!["ru"],
        )
        .unwrap(),
    );
    cfg.data(engine).service((
        web::resource("/suggest").to(super::suggest),
        web::resource("/reverse").to(super::reverse),
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
        .uri("/suggest?pattern=Voronezh&lang=ru")
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
        .uri("/reverse?lat=51.6372&lng=39.1937")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let item = result.get("item").unwrap().as_object().unwrap();
    assert_eq!(item.get("name").unwrap().as_str().unwrap(), "Voronezh");

    Ok(())
}

#[ntex::test]
async fn api_reverse_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/reverse?lat=51.6372&lng=39.1937&lang=ru")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let item = result.get("item").unwrap().as_object().unwrap();
    assert_eq!(item.get("name").unwrap().as_str().unwrap(), "Воронеж");

    Ok(())
}
