use std::sync::Arc;
use std::time::Instant;

use ntex::web::{self, middleware, App, HttpRequest, HttpResponse};
use ntex_cors::Cors;
use ntex_files as fs;
use serde::{Deserialize, Serialize};

use geosuggest_core::{CitiesRecord, Engine};

// openapi3
use oaph::{
    schemars::{self, JsonSchema},
    OpenApiPlaceHolder,
};

mod settings;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestQuery {
    pattern: String,
    limit: Option<usize>,
    /// isolanguage code
    lang: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReverseQuery {
    lat: f64,
    lng: f64,
    /// isolanguage code
    lang: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct SuggestResult<'a> {
    items: Vec<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct ReverseResult<'a> {
    item: Option<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct CityResultItem<'a> {
    id: usize,
    name: &'a str,
    country_code: &'a str,
    timezone: &'a str,
    latitude: f64,
    longitude: f64,
}

impl<'a> CityResultItem<'a> {
    pub fn from_city(item: &'a CitiesRecord, lang: Option<&'a str>) -> Self {
        let name = match (lang, item.names.as_ref()) {
            (Some(lang), Some(names)) => names.get(lang).unwrap_or(&item.name),
            _ => &item.name,
        };
        CityResultItem {
            id: item.id,
            name,
            country_code: &item.country_code,
            timezone: &item.timezone,
            latitude: item.latitude,
            longitude: item.longitude,
        }
    }
}

pub async fn suggest(
    engine: web::types::Data<Arc<Engine>>,
    web::types::Query(suggest_query): web::types::Query<SuggestQuery>,
    _req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let result = engine
        .suggest(
            suggest_query.pattern.as_str(),
            suggest_query.limit.unwrap_or(10),
        )
        .iter()
        .map(|item| CityResultItem::from_city(item, suggest_query.lang.as_deref()))
        .collect::<Vec<CityResultItem>>();
    HttpResponse::Ok().json(&SuggestResult {
        time: now.elapsed().as_millis() as usize,
        items: result,
    })
}

pub async fn reverse(
    engine: web::types::Data<Arc<Engine>>,
    web::types::Query(reverse_query): web::types::Query<ReverseQuery>,
    _req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let city = engine.reverse((reverse_query.lat, reverse_query.lng));

    HttpResponse::Ok().json(&ReverseResult {
        time: now.elapsed().as_millis() as usize,
        item: city.map(|city| CityResultItem::from_city(city, reverse_query.lang.as_deref())),
    })
}

fn generate_openapi_files() -> Result<(), Box<dyn std::error::Error>> {
    let openapi3_yaml_path = std::env::temp_dir().join("openapi3.yaml");

    // render openapi3 yaml to temporary file
    OpenApiPlaceHolder::new()
        .query_params::<SuggestQuery>("SuggestQuery")?
        .query_params::<ReverseQuery>("ReverseQuery")?
        .schema::<SuggestResult>("SuggestResult")?
        .schema::<ReverseResult>("ReverseResult")?
        .render_to_file(include_str!("openapi3.yaml"), &openapi3_yaml_path)?;

    log::info!("openapi3 file: {:?}", openapi3_yaml_path.to_str());

    // render swagger ui html to temporary file
    OpenApiPlaceHolder::swagger_ui_html_to_file(
        "/openapi3.yaml",
        std::env::temp_dir().join("swagger-ui.html"),
    )?;

    // render redoc ui html to temporary file
    OpenApiPlaceHolder::redoc_ui_html_to_file(
        "/openapi3.yaml",
        std::env::temp_dir().join("redoc-ui.html"),
    )?;

    Ok(())
}

#[ntex::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let settings = settings::Settings::new().expect("On read settings");
    log::info!("Settings are:\n{:#?}", settings);

    // generate files for openapi3.yaml and swagger ui
    generate_openapi_files().expect("On generate openapi3 files");

    let shared_engine = Arc::new(
        Engine::load_from_json(&settings.index_file).unwrap_or_else(|e| {
            panic!("On build engine from file: {} - {}", settings.index_file, e)
        }),
    );

    let shared_engine_clone = shared_engine.clone();
    let settings_clone = settings.clone();

    web::server(move || {
        let shared_engine = shared_engine_clone.clone();
        let settings = settings_clone.clone();

        App::new()
            // enable logger
            .data(shared_engine)
            .wrap(middleware::Logger::default())
            .wrap(Cors::default())
            .service((
                // api
                web::resource("/api/city/suggest").to(suggest),
                web::resource("/api/city/reverse").to(reverse),
                // serve openapi3 yaml and ui from files
                fs::Files::new("/openapi3.yaml", std::env::temp_dir()).index_file("openapi3.yaml"),
                fs::Files::new("/swagger", std::env::temp_dir()).index_file("swagger-ui.html"),
                fs::Files::new("/redoc", std::env::temp_dir()).index_file("redoc-ui.html"),
            ))
            .configure(move |cfg: &mut web::ServiceConfig| {
                if let Some(static_dir) = settings.static_dir.as_ref() {
                    cfg.service(fs::Files::new("/", static_dir).index_file("index.html"));
                }
            })
    })
    .bind(format!("{}:{}", settings.host, settings.port))?
    .run()
    .await
}

#[cfg(test)]
mod tests;
