use geosuggest_core::{
    index::{IndexData, SourceFileOptions},
    EngineData,
};
use ntex::web::{test, App, Error, ServiceConfig};
use ntex::{http, web};

use std::sync::Arc;

fn app_config(cfg: &mut ServiceConfig) {
    let data = IndexData::new_from_files(SourceFileOptions {
        cities: "../geosuggest-core/tests/misc/cities.txt",
        names: Some("../geosuggest-core/tests/misc/names.txt"),
        countries: Some("../geosuggest-core/tests/misc/country-info.txt"),
        filter_languages: vec!["ru"],
        admin1_codes: Some("../geosuggest-core/tests/misc/admin1-codes.txt"),
        admin2_codes: Some("../geosuggest-core/tests/misc/admin2-codes.txt"),
    })
    .unwrap();

    let mut engine_data = EngineData::try_from(data).unwrap();

    #[cfg(feature = "geoip2")]
    engine_data
        .load_geoip2("../geosuggest-core/tests/misc/GeoLite2-City-Test.mmdb")
        .unwrap();

    // build static engine
    let engine_data = Box::new(engine_data);
    let engine_data = Box::leak(engine_data);
    let engine = engine_data
        .as_engine()
        .expect("Failed to initialize engine");
    let engine = Box::new(engine);
    let static_engine = Box::leak(engine);
    let shared_engine = Arc::new(static_engine);

    cfg.state(shared_engine).service((
        web::resource("/get").to(super::city_get),
        web::resource("/capital").to(super::capital),
        web::resource("/suggest").to(super::suggest),
        web::resource("/reverse").to(super::reverse),
        #[cfg(feature = "geoip2")]
        web::resource("/geoip2").to(super::geoip2),
    ));
}

#[test_log::test(ntex::test)]
async fn api_get() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get().uri("/get?id=472045").to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city");
    assert!(city.is_some());
    let city = city.unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "Voronezh");

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_capital_country_code() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/capital?country_code=RU")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city");
    assert!(city.is_some());
    let city = city.unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "Moscow");

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_capital_coordinates() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/capital?lat=55.7558&lng=37.6173&country_code=GB")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;
    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city").unwrap().as_object().unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "Moscow");

    Ok(())
}

#[cfg(feature = "geoip2")]
#[test_log::test(ntex::test)]
async fn api_capital_ip() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/capital?ip=81.2.69.142&country_code=RU")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;
    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city").unwrap().as_object().unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "London");

    Ok(())
}

#[cfg(feature = "geoip2")]
#[test_log::test(ntex::test)]
async fn api_capital_ip_client() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/capital?ip=client&country_code=RU")
        .header(ntex::http::header::FORWARDED, "81.2.69.142")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;
    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city").unwrap().as_object().unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "London");

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_get_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/get?id=472045&lang=ru")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let city = result.get("city");
    assert!(city.is_some());
    let city = city.unwrap();
    assert_eq!(city.get("name").unwrap().as_str().unwrap(), "Воронеж");

    assert_eq!(
        city.get("country")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Россия"
    );
    assert_eq!(
        city.get("admin_division")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Воронежская область"
    );

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_suggest() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/suggest?pattern=Voronezh&countries=RU,JP")
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

#[test_log::test(ntex::test)]
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
    assert_eq!(
        items[0]
            .get("country")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Россия"
    );
    assert_eq!(
        items[0]
            .get("admin_division")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Воронежская область"
    );

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_reverse() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/reverse?lat=51.6372&lng=39.1937&limit=1&countries=RU,JP")
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

#[test_log::test(ntex::test)]
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
    assert_eq!(
        items[0]
            .get("city")
            .unwrap()
            .as_object()
            .unwrap()
            .get("country")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Россия"
    );
    assert_eq!(
        items[0]
            .get("city")
            .unwrap()
            .as_object()
            .unwrap()
            .get("admin_division")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Воронежская область"
    );

    Ok(())
}

#[cfg(feature = "geoip2")]
#[test_log::test(ntex::test)]
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

#[test_log::test(ntex::test)]
async fn api_suggest_admin2_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/suggest?pattern=Beverley&lang=ru&limit=1")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert!(!items.is_empty());
    assert_eq!(items[0].get("name").unwrap().as_str().unwrap(), "Beverley");
    assert_eq!(
        items[0]
            .get("admin2_division")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Ист-Райдинг-оф-Йоркшир"
    );

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_reverse_admin2_lang() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/reverse?lat=53.84587&lng=-0.42332&lang=ru&limit=1")
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
        "Beverley"
    );
    assert_eq!(
        items[0]
            .get("city")
            .unwrap()
            .as_object()
            .unwrap()
            .get("admin2_division")
            .unwrap()
            .as_object()
            .unwrap()
            .get("name")
            .unwrap()
            .as_str()
            .unwrap(),
        "Ист-Райдинг-оф-Йоркшир"
    );

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_suggest_filter_by_countries() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/suggest?pattern=Voronezh&countries=JP,KR")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert!(items.is_empty());

    Ok(())
}

#[test_log::test(ntex::test)]
async fn api_reverse_filter_by_countries() -> Result<(), Error> {
    let app = test::init_service(App::new().configure(app_config)).await;

    let req = test::TestRequest::get()
        .uri("/reverse?lat=51.6372&lng=39.1937&limit=1&countries=JP,KR")
        .to_request();
    let resp = app.call(req).await.unwrap();

    assert_eq!(resp.status(), http::StatusCode::OK);

    let bytes = test::read_body(resp).await;

    let result: serde_json::Value = serde_json::from_slice(bytes.as_ref())?;
    let items = result.get("items").unwrap().as_array().unwrap();
    assert!(items.is_empty());

    Ok(())
}
