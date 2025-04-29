use std::boxed::Box;
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "tracing")]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "geoip2")]
use std::net::IpAddr;
#[cfg(feature = "geoip2")]
use std::str::FromStr;

use ntex::web::{self, middleware, App, HttpRequest, HttpResponse};
use ntex_cors::Cors;
use ntex_files as fs;
use serde::{Deserialize, Serialize};

use geosuggest_core::{storage, CitiesRecord, Engine};

// openapi3
use oaph::{
    schemars::{self, JsonSchema},
    OpenApiPlaceHolder,
};

mod settings;

const DEFAULT_K: f32 = 0.000000005;
const DEFAULT_NEAREST_CITIES_LIMIT: usize = 10;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCityQuery {
    /// geonameid of the City
    id: u32,
    /// isolanguage code
    lang: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCapitalQuery {
    /// geonameid of the City
    country_code: String,
    /// isolanguage code
    lang: Option<String>,
}

// TODO self.countries.split(",").as_slice()
// https://github.com/rust-lang/rust/issues/96137
fn get_countries_filter(countries: &Option<String>) -> Option<Vec<&str>> {
    countries.as_deref().map(|c| c.split(',').collect())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestQuery {
    pattern: String,
    limit: Option<usize>,
    /// isolanguage code
    lang: Option<String>,
    /// min score of Jaro Winkler similarity (by default 0.8)
    min_score: Option<f32>,
    /// comma separated country code (2-letter) to pre-filter search
    countries: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReverseQuery {
    lat: f32,
    lng: f32,
    limit: Option<usize>,
    /// isolanguage code
    lang: Option<String>,
    /// distance correction coefficient by city population `score(item) = item.distance - k * item.city.population`
    /// by default `0.000000005`
    k: Option<f32>,
    /// neareset cities to apply distance correction coefficient by population
    /// by default 10
    nearest_limit: Option<usize>,
    /// comma separated country code (2-letter) to pre-filter search
    countries: Option<String>,
}

#[cfg(feature = "geoip2")]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GeoIP2Query {
    /// IP to check, if not declared then `Forwarded` header will used or peer ip as last chance
    ip: Option<String>,
    /// isolanguage code
    lang: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct GetCityResult<'a> {
    city: Option<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct GetCapitalResult<'a> {
    city: Option<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SuggestResult<'a> {
    items: Vec<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct ReverseResult<'a> {
    items: Vec<ReverseResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct ReverseResultItem<'a> {
    city: CityResultItem<'a>,
    distance: f32,
    score: f32,
}

#[derive(Serialize, JsonSchema)]
pub struct CountryItem<'a> {
    id: u32,
    code: &'a str,
    name: &'a str,
}

#[derive(Serialize, JsonSchema)]
pub struct AdminDivisionItem<'a> {
    id: u32,
    code: &'a str,
    name: &'a str,
}

#[derive(Serialize, JsonSchema)]
pub struct CityResultItem<'a> {
    id: u32,
    name: &'a str,
    country: Option<CountryItem<'a>>,
    admin_division: Option<AdminDivisionItem<'a>>,
    admin2_division: Option<AdminDivisionItem<'a>>,
    timezone: &'a str,
    latitude: f32,
    longitude: f32,
    population: u32,
}

#[cfg(feature = "geoip2")]
#[derive(Serialize, JsonSchema)]
pub struct GeoIP2Result<'a> {
    city: Option<CityResultItem<'a>>,
    for_ip: String,
    /// elapsed time in ms
    time: usize,
}

impl<'a> CityResultItem<'a> {
    pub fn from_city(item: &'a CitiesRecord, lang: Option<&'a str>) -> Self {
        let name = match (lang, item.names.as_ref()) {
            (Some(lang), Some(names)) => names.get(lang).unwrap_or(&item.name),
            _ => &item.name,
        };

        let country = if let Some(ref country) = item.country {
            let country_name = match (lang, item.country_names.as_ref()) {
                (Some(lang), Some(names)) => names.get(lang).unwrap_or(&country.name),
                _ => &country.name,
            };
            Some(CountryItem {
                id: country.id,
                code: &country.code,
                name: country_name,
            })
        } else {
            None
        };

        let admin_division = if let Some(ref admin1) = item.admin_division {
            let admin1_name = match (lang, item.admin1_names.as_ref()) {
                (Some(lang), Some(names)) => names.get(lang).unwrap_or(&admin1.name),
                _ => &admin1.name,
            };
            Some(AdminDivisionItem {
                id: admin1.id,
                code: &admin1.code,
                name: admin1_name,
            })
        } else {
            None
        };

        let admin2_division = if let Some(ref admin2) = item.admin2_division {
            let admin2_name = match (lang, item.admin2_names.as_ref()) {
                (Some(lang), Some(names)) => names.get(lang).unwrap_or(&admin2.name),
                _ => &admin2.name,
            };
            Some(AdminDivisionItem {
                id: admin2.id,
                code: &admin2.code,
                name: admin2_name,
            })
        } else {
            None
        };

        CityResultItem {
            id: item.id,
            name,
            country,
            admin_division,
            admin2_division,
            timezone: &item.timezone,
            latitude: item.latitude,
            longitude: item.longitude,
            population: item.population,
        }
    }
}

pub async fn city_get(
    engine: web::types::State<Arc<Engine>>,
    web::types::Query(query): web::types::Query<GetCityQuery>,
    _req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let city = engine
        .get(&query.id)
        .map(|city| CityResultItem::from_city(city, query.lang.as_deref()));

    HttpResponse::Ok().json(&GetCityResult {
        time: now.elapsed().as_millis() as usize,
        city,
    })
}

pub async fn capital(
    engine: web::types::State<Arc<Engine>>,
    web::types::Query(query): web::types::Query<GetCapitalQuery>,
    _req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let city = engine
        .capital(&query.country_code)
        .map(|city| CityResultItem::from_city(city, query.lang.as_deref()));

    HttpResponse::Ok().json(&GetCapitalResult {
        time: now.elapsed().as_millis() as usize,
        city,
    })
}

pub async fn suggest(
    engine: web::types::State<Arc<Engine>>,
    web::types::Query(query): web::types::Query<SuggestQuery>,
    _req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let result = engine
        .suggest(
            query.pattern.as_str(),
            query.limit.unwrap_or(10),
            query.min_score,
            get_countries_filter(&query.countries).as_deref(),
        )
        .iter()
        .map(|item| CityResultItem::from_city(item, query.lang.as_deref()))
        .collect::<Vec<CityResultItem>>();

    HttpResponse::Ok().json(&SuggestResult {
        time: now.elapsed().as_millis() as usize,
        items: result,
    })
}

pub async fn reverse(
    engine: web::types::State<Arc<Engine>>,
    web::types::Query(query): web::types::Query<ReverseQuery>,
    _req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let items = engine
        .reverse(
            (query.lat, query.lng),
            query.nearest_limit.unwrap_or(DEFAULT_NEAREST_CITIES_LIMIT),
            Some(query.k.unwrap_or(DEFAULT_K)),
            get_countries_filter(&query.countries).as_deref(),
        )
        .unwrap_or_default();

    HttpResponse::Ok().json(&ReverseResult {
        time: now.elapsed().as_millis() as usize,
        items: items
            .iter()
            .take(query.limit.unwrap_or(DEFAULT_NEAREST_CITIES_LIMIT))
            .map(|item| ReverseResultItem {
                city: CityResultItem::from_city(item.city, query.lang.as_deref()),
                distance: item.distance,
                score: item.score,
            })
            .collect(),
    })
}

#[cfg(feature = "geoip2")]
pub async fn geoip2(
    engine: web::types::State<Arc<Engine>>,
    web::types::Query(query): web::types::Query<GeoIP2Query>,
    req: HttpRequest,
) -> HttpResponse {
    let now = Instant::now();

    let ip = match query.ip.as_ref() {
        Some(ip) => Some(ip.as_str()),
        None => {
            // fallback to headers
            if let Some(forwarded) = req.headers().get(ntex::http::header::FORWARDED) {
                forwarded.to_str().ok()
            } else {
                None
            }
        }
    };

    let addr = match ip {
        Some(ip) => match IpAddr::from_str(ip) {
            Ok(addr) => addr,
            Err(e) => {
                return HttpResponse::BadRequest()
                    .body(format!("Invalid ip addr: {} error: {}", ip, e))
            }
        },
        None => {
            if let Some(v) = req.connection_info().remote() {
                if let Ok(ip) = IpAddr::from_str(v.split(':').take(1).next().unwrap_or("")) {
                    ip
                } else {
                    return HttpResponse::BadRequest().body(
                        "IP address is not declared in request and field to get peer addr"
                            .to_string(),
                    );
                }
            } else if let Some(peer_addr) = req.peer_addr() {
                peer_addr.ip()
            } else {
                return HttpResponse::BadRequest().body(
                    "IP address is not declared in request and field to get peer addr".to_string(),
                );
            }
        }
    };

    let result = engine.geoip2_lookup(addr);

    HttpResponse::Ok().json(&GeoIP2Result {
        time: now.elapsed().as_millis() as usize,
        for_ip: addr.to_string(),
        city: result.map(|item| CityResultItem::from_city(item, query.lang.as_deref())),
    })
}

fn generate_openapi_files(settings: &settings::Settings) -> Result<(), Box<dyn std::error::Error>> {
    let openapi3_yaml_path = std::env::temp_dir().join("openapi3.yaml");

    // render openapi3 yaml to temporary file
    let aoph = OpenApiPlaceHolder::new()
        .substitute("version", VERSION)
        .substitute("url_path_prefix", &settings.url_path_prefix)
        .query_params::<GetCityQuery>("GetCityQuery")?
        .query_params::<GetCapitalQuery>("GetCapitalQuery")?
        .query_params::<SuggestQuery>("SuggestQuery")?
        .query_params::<ReverseQuery>("ReverseQuery")?
        .schema::<GetCityResult>("GetCityResult")?
        .schema::<GetCapitalResult>("GetCapitalResult")?
        .schema::<SuggestResult>("SuggestResult")?
        .schema::<ReverseResult>("ReverseResult")?;

    #[cfg(feature = "geoip2")]
    let aoph = {
        aoph.query_params::<GeoIP2Query>("GeoIP2Query")?
            .schema::<GeoIP2Result>("GeoIP2Result")?
    };

    aoph.render_to_file(include_str!("openapi3.yaml"), &openapi3_yaml_path)?;

    #[cfg(feature = "tracing")]
    tracing::info!("openapi3 file: {:?}", openapi3_yaml_path.to_str());

    let title = format!("geosuggest v{}", VERSION);

    let openapi3_url_path = std::path::Path::new(&settings.url_path_prefix).join("openapi3.yaml");
    let openapi3_url_path = openapi3_url_path
        .to_str()
        .ok_or("Failed to build openapi3 url")?;

    // render swagger ui html to temporary file
    OpenApiPlaceHolder::swagger_ui_html_to_file(
        openapi3_url_path,
        &title,
        std::env::temp_dir().join("swagger-ui.html"),
    )?;

    // render redoc ui html to temporary file
    OpenApiPlaceHolder::redoc_ui_html_to_file(
        openapi3_url_path,
        &title,
        std::env::temp_dir().join("redoc-ui.html"),
    )?;

    Ok(())
}

#[ntex::main]
async fn main() -> std::io::Result<()> {
    // logging
    #[cfg(feature = "tracing")]
    {
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
            ))
            .with(tracing_subscriber::fmt::layer());
        subscriber.init();
    }

    let settings = settings::Settings::new().expect("On read settings");
    #[cfg(feature = "tracing")]
    tracing::info!("Settings are:\n{:#?}", settings);

    // generate files for openapi3.yaml and swagger ui
    generate_openapi_files(&settings).expect("On generate openapi3 files");

    if settings.index_file.is_empty() {
        panic!("Please set `index_file`");
    }

    let storage = storage::Storage::new();

    let mut engine = storage
        .load_from(&settings.index_file)
        .unwrap_or_else(|e| panic!("On build engine from file: {} - {}", settings.index_file, e));

    #[cfg(feature = "geoip2")]
    if let Some(geoip2_file) = settings.geoip2_file.as_ref() {
        engine
            .load_geoip2(geoip2_file)
            .unwrap_or_else(|_| panic!("On read geoip2 file from {}", geoip2_file));
    }

    let shared_engine = Arc::new(engine);
    let shared_engine_clone = shared_engine.clone();

    let settings_clone = settings.clone();

    let listen_on = format!("{}:{}", settings.host, settings.port);
    #[cfg(feature = "tracing")]
    tracing::info!("Listen on {}", listen_on);

    web::server(move || {
        let shared_engine = shared_engine_clone.clone();
        let settings = settings_clone.clone();

        App::new()
            .state(shared_engine)
            // enable logger
            .wrap(middleware::Logger::default())
            .wrap(Cors::default())
            .service(
                web::scope(&settings.url_path_prefix)
                    .service((
                        // api
                        web::resource("/api/city/get").to(city_get),
                        web::resource("/api/city/capital").to(capital),
                        web::resource("/api/city/suggest").to(suggest),
                        web::resource("/api/city/reverse").to(reverse),
                        #[cfg(feature = "geoip2")]
                        web::resource("/api/city/geoip2").to(geoip2),
                        // serve openapi3 yaml and ui from files
                        fs::Files::new("/openapi3.yaml", std::env::temp_dir())
                            .index_file("openapi3.yaml"),
                        fs::Files::new("/swagger", std::env::temp_dir())
                            .index_file("swagger-ui.html"),
                        fs::Files::new("/redoc", std::env::temp_dir()).index_file("redoc-ui.html"),
                    ))
                    .configure(move |cfg: &mut web::ServiceConfig| {
                        if let Some(static_dir) = settings.static_dir.as_ref() {
                            cfg.service(fs::Files::new("/", static_dir).index_file("index.html"));
                        }
                    }),
            )
    })
    .bind(listen_on)?
    .run()
    .await
}

#[cfg(test)]
mod tests;
