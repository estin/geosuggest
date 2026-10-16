use std::boxed::Box;
#[cfg(feature = "geoip2")]
use std::net::IpAddr;
#[cfg(feature = "geoip2")]
use std::str::FromStr;
use std::time::Instant;

#[cfg(feature = "tracing")]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use serde::{Deserialize, Serialize};
#[cfg(feature = "demo")]
use topcoat::asset::{AssetBundle, AssetConfig, RouterBuilderAssetExt};
#[cfg(any(feature = "demo", feature = "swagger"))]
use topcoat::router::content::Html;
#[cfg(feature = "demo")]
use topcoat::router::RouterBuilderDirectoryExt;
#[cfg(feature = "demo")]
use topcoat::runtime::RouterBuilderRuntimeExt;
use topcoat::{
    context::{app_context, Cx},
    router::{
        self,
        response::{IntoResponse, Response},
        Body, LayerFn, LayerFuture, Method, Next, PathBuf as RouterPathBuf, PathSegment, RouteFn,
        RouteFuture, Router, StatusCode,
    },
    Result,
};

use geosuggest_core::{index::ArchivedCitiesRecord, storage, Engine};

#[cfg(feature = "swagger")]
use oaph::{
    schemars::{self, JsonSchema},
    OpenApiPlaceHolder,
};

mod settings;

const DEFAULT_K: f32 = 0.000000005;
const DEFAULT_NEAREST_CITIES_LIMIT: usize = 10;
#[cfg(feature = "swagger")]
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type SharedEngine = &'static Engine<'static>;

#[derive(Clone)]
pub struct AppState {
    pub engine: SharedEngine,
    pub settings: settings::Settings,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct GetCityQuery {
    /// geonameid of the City
    id: u32,
    /// isolanguage code
    lang: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct GetCapitalQuery {
    lat: Option<f32>,
    lng: Option<f32>,
    /// IP to check, if not declared then `Forwarded` header or peer IP will be used
    #[cfg(feature = "geoip2")]
    ip: Option<String>,
    /// geonameid of the City
    country_code: Option<String>,
    /// isolanguage code
    lang: Option<String>,
}

pub enum GetCapitalLookup<'a> {
    Coords {
        lat: f32,
        lng: f32,
    },
    CountryCode(&'a str),
    #[cfg(feature = "geoip2")]
    Ip(&'a str),
}

impl GetCapitalQuery {
    fn lookups<'a>(&'a self) -> impl Iterator<Item = GetCapitalLookup<'a>> {
        [
            self.lat
                .zip(self.lng)
                .map(|(lat, lng)| GetCapitalLookup::Coords { lat, lng }),
            #[cfg(feature = "geoip2")]
            self.ip.as_deref().map(GetCapitalLookup::Ip),
            self.country_code
                .as_deref()
                .map(GetCapitalLookup::CountryCode),
        ]
        .into_iter()
        .flatten()
    }
}

fn get_countries_filter(countries: &Option<String>) -> Option<Vec<&str>> {
    countries.as_deref().map(|c| c.split(',').collect())
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
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

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
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
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct GeoIP2Query {
    /// IP to check, if not declared then `Forwarded` header will used or peer ip as last chance
    ip: Option<String>,
    /// isolanguage code
    lang: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct GetCityResult<'a> {
    city: Option<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct GetCapitalResult<'a> {
    city: Option<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct SuggestResult<'a> {
    items: Vec<CityResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct ReverseResult<'a> {
    items: Vec<ReverseResultItem<'a>>,
    /// elapsed time in ms
    time: usize,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct ReverseResultItem<'a> {
    city: CityResultItem<'a>,
    distance: f32,
    score: f32,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct CountryItem<'a> {
    id: u32,
    code: &'a str,
    name: &'a str,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct AdminDivisionItem<'a> {
    id: u32,
    code: &'a str,
    name: &'a str,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
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
#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(JsonSchema))]
pub struct GeoIP2Result<'a> {
    city: Option<CityResultItem<'a>>,
    for_ip: String,
    /// elapsed time in ms
    time: usize,
}

impl<'a> CityResultItem<'a> {
    pub fn from_city<'b>(
        engine: &'a Engine<'a>,
        item: &'a ArchivedCitiesRecord,
        lang: Option<&'b str>,
    ) -> Self {
        let name = match (lang, item.names.as_ref()) {
            (Some(lang), Some(names)) => names.get(lang).unwrap_or(&item.name),
            _ => &item.name,
        };

        let country = if let Some(country) = engine.city_country(item) {
            let country_name = match lang {
                Some(lang) => engine
                    .country_names(&country.code)
                    .and_then(|names| names.get(lang))
                    .map(|v| v.as_str())
                    .unwrap_or(country.name.as_str()),
                None => &country.name,
            };
            Some(CountryItem {
                id: country.id.to_native(),
                code: &country.code,
                name: country_name,
            })
        } else {
            None
        };

        let admin_division = if let Some(admin1) = engine.city_admin1(item) {
            let admin1_name = match lang {
                Some(lang) => engine
                    .admin1_names(admin1.id.to_native())
                    .and_then(|names| names.get(lang))
                    .unwrap_or(&admin1.name),
                None => &admin1.name,
            };
            Some(AdminDivisionItem {
                id: admin1.id.to_native(),
                code: &admin1.code,
                name: admin1_name,
            })
        } else {
            None
        };

        let admin2_division = if let Some(admin2) = engine.city_admin2(item) {
            let admin2_name = match lang {
                Some(lang) => engine
                    .admin2_names(admin2.id.to_native())
                    .and_then(|names| names.get(lang))
                    .unwrap_or(&admin2.name),
                None => &admin2.name,
            };
            Some(AdminDivisionItem {
                id: admin2.id.to_native(),
                code: &admin2.code,
                name: admin2_name,
            })
        } else {
            None
        };

        CityResultItem {
            id: item.id.to_native(),
            name,
            country,
            admin_division,
            admin2_division,
            timezone: engine.city_timezone(item),
            latitude: item.latitude.to_native(),
            longitude: item.longitude.to_native(),
            population: item.population.to_native(),
        }
    }
}

#[cfg(feature = "geoip2")]
fn extract_ip_addr(ip_param: Option<&str>, cx: &Cx) -> Result<IpAddr, String> {
    let parts = router::request::parts(cx);
    let headers = &parts.headers;
    ip_param
        .filter(|ip| *ip != "client")
        .or_else(|| {
            headers
                .get(topcoat::router::header::FORWARDED)
                .and_then(|val| val.to_str().ok())
        })
        .map(|ip_str| {
            IpAddr::from_str(ip_str)
                .map_err(|err| format!("Invalid ip addr: {ip_str} error: {err}"))
        })
        .or_else(|| router::request::client_ip(cx).map(Ok))
        .ok_or_else(|| "IP address not provided".to_owned())
        .flatten()
}

fn get_country_code<'a>(
    engine: &'a Engine<'a>,
    lookup: GetCapitalLookup<'a>,
    _cx: &Cx,
) -> Result<Option<&'a str>, String> {
    Ok(match lookup {
        GetCapitalLookup::Coords { lat, lng } => engine
            .reverse::<&str>((lat, lng), 1, None, None)
            .and_then(|items| items.into_iter().next())
            .and_then(|item| engine.city_country(item.city))
            .map(|country| country.code.as_str()),
        #[cfg(feature = "geoip2")]
        GetCapitalLookup::Ip(ip) => {
            let addr = extract_ip_addr(Some(ip), _cx)?;
            engine
                .geoip2_lookup(addr)
                .and_then(|city| engine.city_country(city))
                .map(|country| country.code.as_str())
        }
        GetCapitalLookup::CountryCode(country_code) => Some(country_code),
    })
}

async fn city_get(cx: &Cx) -> Result<router::content::Json<GetCityResult<'static>>> {
    let app = app_context::<AppState>(cx);
    let engine = app.engine;
    let query: GetCityQuery =
        router::parse_query_params(cx).map_err(|e| router::error::bad_request(e.to_string()))?;
    let now = Instant::now();

    let city = engine
        .get(&query.id)
        .map(|city| CityResultItem::from_city(engine, city, query.lang.as_deref()));

    Ok(router::content::Json(GetCityResult {
        time: now.elapsed().as_millis() as usize,
        city,
    }))
}

async fn capital(cx: &Cx) -> Result<router::content::Json<GetCapitalResult<'static>>> {
    let app = app_context::<AppState>(cx);
    let engine = app.engine;
    let query: GetCapitalQuery =
        router::parse_query_params(cx).map_err(|e| router::error::bad_request(e.to_string()))?;
    let now = Instant::now();

    let mut err = None;

    for lookup in query.lookups() {
        let country_code = match get_country_code(engine, lookup, cx) {
            Ok(Some(country_code)) => country_code,
            Ok(None) => continue,
            Err(e) => {
                err = Some(e);
                continue;
            }
        };

        let Some(city) = engine.capital(country_code) else {
            continue;
        };

        return Ok(router::content::Json(GetCapitalResult {
            time: now.elapsed().as_millis() as usize,
            city: Some(CityResultItem::from_city(
                engine,
                city,
                query.lang.as_deref(),
            )),
        }));
    }

    if let Some(err) = err {
        return Err(router::error::bad_request(err).into());
    }

    Ok(router::content::Json(GetCapitalResult {
        time: now.elapsed().as_millis() as usize,
        city: None,
    }))
}

async fn suggest(cx: &Cx) -> Result<router::content::Json<SuggestResult<'static>>> {
    let app = app_context::<AppState>(cx);
    let engine = app.engine;
    let query: SuggestQuery =
        router::parse_query_params(cx).map_err(|e| router::error::bad_request(e.to_string()))?;
    let now = Instant::now();

    let result = engine
        .suggest(
            query.pattern.as_str(),
            query.limit.unwrap_or(10),
            query.min_score,
            get_countries_filter(&query.countries).as_deref(),
        )
        .iter()
        .map(move |item| CityResultItem::from_city(engine, item, query.lang.as_deref()))
        .collect::<Vec<CityResultItem>>();

    Ok(router::content::Json(SuggestResult {
        time: now.elapsed().as_millis() as usize,
        items: result,
    }))
}

async fn reverse(cx: &Cx) -> Result<router::content::Json<ReverseResult<'static>>> {
    let app = app_context::<AppState>(cx);
    let engine = app.engine;
    let query: ReverseQuery =
        router::parse_query_params(cx).map_err(|e| router::error::bad_request(e.to_string()))?;
    let now = Instant::now();

    let items = engine
        .reverse(
            (query.lat, query.lng),
            query.nearest_limit.unwrap_or(DEFAULT_NEAREST_CITIES_LIMIT),
            Some(query.k.unwrap_or(DEFAULT_K)),
            get_countries_filter(&query.countries).as_deref(),
        )
        .unwrap_or_default();

    Ok(router::content::Json(ReverseResult {
        time: now.elapsed().as_millis() as usize,
        items: items
            .iter()
            .take(query.limit.unwrap_or(DEFAULT_NEAREST_CITIES_LIMIT))
            .map(move |item| ReverseResultItem {
                city: CityResultItem::from_city(engine, item.city, query.lang.as_deref()),
                distance: item.distance,
                score: item.score,
            })
            .collect(),
    }))
}

#[cfg(feature = "geoip2")]
async fn geoip2(cx: &Cx) -> Result<router::content::Json<GeoIP2Result<'static>>> {
    let app = app_context::<AppState>(cx);
    let engine = app.engine;
    let query: GeoIP2Query =
        router::parse_query_params(cx).map_err(|e| router::error::bad_request(e.to_string()))?;
    let now = Instant::now();

    let addr = match extract_ip_addr(query.ip.as_deref(), cx) {
        Ok(addr) => addr,
        Err(err) => return Err(router::error::bad_request(err).into()),
    };

    let result = engine.geoip2_lookup(addr);

    Ok(router::content::Json(GeoIP2Result {
        time: now.elapsed().as_millis() as usize,
        for_ip: addr.to_string(),
        city: result
            .map(move |item| CityResultItem::from_city(engine, item, query.lang.as_deref())),
    }))
}

#[cfg(feature = "demo")]
async fn root_index(cx: &Cx) -> Result<Html<String>> {
    let app = app_context::<AppState>(cx);
    if let Some(dir) = &app.settings.static_dir {
        let path = std::path::PathBuf::from(dir).join("index.html");
        if let Ok(body) = std::fs::read_to_string(path) {
            return Ok(Html(body));
        }
    }
    Ok(Html("geosuggest".to_string()))
}

#[cfg(feature = "swagger")]
async fn openapi3(_cx: &Cx) -> Result<Response> {
    let body = std::fs::read(std::env::temp_dir().join("openapi3.yaml"))?;
    Ok(Response::builder()
        .header("content-type", "application/yaml")
        .body(Body::from(body))?)
}

#[cfg(feature = "swagger")]
async fn swagger(_cx: &Cx) -> Result<Html<String>> {
    let body = std::fs::read_to_string(std::env::temp_dir().join("swagger-ui.html"))?;
    Ok(Html(body))
}

#[cfg(feature = "swagger")]
async fn redoc(_cx: &Cx) -> Result<Html<String>> {
    let body = std::fs::read_to_string(std::env::temp_dir().join("redoc-ui.html"))?;
    Ok(Html(body))
}

#[cfg(feature = "swagger")]
fn generate_openapi_files(settings: &settings::Settings) -> Result<(), Box<dyn std::error::Error>> {
    let openapi3_yaml_path = std::env::temp_dir().join("openapi3.yaml");

    let aoph = OpenApiPlaceHolder::new()
        .substitute("version", VERSION)
        .substitute(
            "url_path_prefix",
            &join_url_path(&settings.url_path_prefix, "/"),
        );
    let aoph = aoph
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
    let openapi3_url_path = join_url_path(&settings.url_path_prefix, "/openapi3.yaml");
    OpenApiPlaceHolder::swagger_ui_html_to_file(
        &openapi3_url_path,
        &title,
        std::env::temp_dir().join("swagger-ui.html"),
    )?;
    OpenApiPlaceHolder::redoc_ui_html_to_file(
        &openapi3_url_path,
        &title,
        std::env::temp_dir().join("redoc-ui.html"),
    )?;

    Ok(())
}

fn join_url_path(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        path.to_owned()
    } else if path == "/" {
        format!("/{prefix}")
    } else {
        format!("/{prefix}{path}")
    }
}

fn route_path(prefix: &str, path: &str) -> RouterPathBuf {
    let joined = join_url_path(prefix, path);
    let mut route_path = RouterPathBuf::new();
    for segment in joined.trim_start_matches('/').split('/') {
        if !segment.is_empty() {
            route_path += PathSegment::new(segment);
        }
    }
    route_path
}

macro_rules! route_adapter {
    ($adapter:ident, $handler:ident) => {
        fn $adapter<'cx>(cx: &'cx Cx, _body: Body) -> RouteFuture<'cx> {
            Box::pin(async move { $handler(cx).await?.into_response(cx) })
        }
    };
}

route_adapter!(city_get_route, city_get);
route_adapter!(capital_route, capital);
route_adapter!(suggest_route, suggest);
route_adapter!(reverse_route, reverse);
#[cfg(feature = "geoip2")]
route_adapter!(geoip2_route, geoip2);
#[cfg(feature = "demo")]
route_adapter!(root_index_route, root_index);
#[cfg(feature = "swagger")]
route_adapter!(swagger_route, swagger);
#[cfg(feature = "swagger")]
route_adapter!(redoc_route, redoc);
#[cfg(feature = "swagger")]
route_adapter!(openapi3_route, openapi3);

fn cors_layer<'cx>(cx: &'cx Cx, body: Body, next: Next<'cx>) -> LayerFuture<'cx> {
    Box::pin(async move {
        use topcoat::router::header::{
            ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
            ACCESS_CONTROL_REQUEST_METHOD, ORIGIN, VARY,
        };
        use topcoat::router::response::response_headers;

        let parts = router::request::parts(cx);
        let origin = parts.headers.get(ORIGIN).cloned();
        let requested_method = parts.headers.get(ACCESS_CONTROL_REQUEST_METHOD).cloned();
        let requested_headers = parts.headers.get(ACCESS_CONTROL_REQUEST_HEADERS).cloned();
        let preflight = parts.method == Method::OPTIONS && requested_method.is_some();

        if let Some(origin) = origin {
            response_headers(cx).append(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            response_headers(cx).append(VARY, "Origin".parse().expect("valid Vary value"));
        }
        if let Some(method) = requested_method {
            response_headers(cx).append(ACCESS_CONTROL_ALLOW_METHODS, method);
        }
        if let Some(requested) = requested_headers {
            response_headers(cx).append(ACCESS_CONTROL_ALLOW_HEADERS, requested);
            response_headers(cx).append(
                VARY,
                "Access-Control-Request-Headers"
                    .parse()
                    .expect("valid Vary value"),
            );
        }

        if preflight {
            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())?)
        } else {
            next.run(cx, body).await
        }
    })
}

#[cfg(feature = "demo")]
type DemoAssetConfig = Option<AssetConfig>;
#[cfg(not(feature = "demo"))]
type DemoAssetConfig = ();

pub fn build_router(app_state: AppState, settings: &settings::Settings) -> Router {
    #[cfg(feature = "demo")]
    let assets = Some(AssetConfig::serve(AssetBundle::load().expect(
        "Topcoat asset bundle is required when the demo feature is enabled",
    )));
    #[cfg(not(feature = "demo"))]
    let assets = ();

    build_router_with_assets(app_state, settings, assets)
}

fn build_router_with_assets(
    app_state: AppState,
    settings: &settings::Settings,
    _assets: DemoAssetConfig,
) -> Router {
    let prefix = settings.url_path_prefix.trim_matches('/');
    let mut builder = Router::builder()
        .route(RouteFn::new(
            Method::GET,
            route_path(prefix, "/api/city/get"),
            city_get_route,
        ))
        .route(RouteFn::new(
            Method::GET,
            route_path(prefix, "/api/city/capital"),
            capital_route,
        ))
        .route(RouteFn::new(
            Method::GET,
            route_path(prefix, "/api/city/suggest"),
            suggest_route,
        ))
        .route(RouteFn::new(
            Method::GET,
            route_path(prefix, "/api/city/reverse"),
            reverse_route,
        ));
    #[cfg(feature = "swagger")]
    {
        builder = builder
            .route(RouteFn::new(
                Method::GET,
                route_path(prefix, "/openapi3.yaml"),
                openapi3_route,
            ))
            .route(RouteFn::new(
                Method::GET,
                route_path(prefix, "/swagger"),
                swagger_route,
            ))
            .route(RouteFn::new(
                Method::GET,
                route_path(prefix, "/redoc"),
                redoc_route,
            ));
    }
    #[cfg(feature = "demo")]
    {
        let root_path = route_path(prefix, "/");
        if settings.static_dir.is_some() {
            builder = builder.route(RouteFn::new(Method::GET, root_path, root_index_route));
        } else {
            builder = builder.page(geosuggest_demo::home_page_at(join_url_path(prefix, "/")));
        }
        builder = builder
            .route(geosuggest_demo::suggest_results)
            .route(geosuggest_demo::reverse_results)
            .route(geosuggest_demo::city_detail);
        if let Some(dir) = settings.static_dir.clone() {
            let static_path = route_path(prefix, "/{*file}");
            builder = builder.serve_dir(static_path, dir);
        }
    }
    #[cfg(feature = "geoip2")]
    {
        builder = builder.route(RouteFn::new(
            Method::GET,
            route_path(prefix, "/api/city/geoip2"),
            geoip2_route,
        ));
    }
    builder = builder.layer(LayerFn::new(None::<&str>, cors_layer));
    #[cfg(feature = "demo")]
    {
        let engine_context = geosuggest_demo::EngineContext {
            engine: app_state.engine,
            url_path_prefix: settings.url_path_prefix.clone(),
        };
        builder = builder.runtime().app_context(engine_context);
        if let Some(assets) = _assets {
            builder = builder.assets(assets);
        }
    }
    builder = builder.app_context(app_state);
    builder.build()
}

pub fn router(app_state: AppState, settings: &settings::Settings) -> Router {
    build_router(app_state, settings)
}

pub fn routes(settings: &settings::Settings, app_state: AppState) -> Router {
    build_router(app_state, settings)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
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

    #[cfg(feature = "swagger")]
    generate_openapi_files(&settings).expect("On generate openapi3 files");

    if settings.index_file.is_empty() {
        panic!("Please set `index_file`");
    }

    let storage = storage::Storage::new();

    let engine_data = storage
        .load_from(&settings.index_file)
        .unwrap_or_else(|e| panic!("On build engine from file: {} - {}", settings.index_file, e));

    #[cfg(feature = "geoip2")]
    let mut engine_data = engine_data;
    #[cfg(feature = "geoip2")]
    if let Some(geoip2_file) = settings.geoip2_file.as_ref() {
        engine_data
            .load_geoip2(geoip2_file)
            .unwrap_or_else(|_| panic!("On read geoip2 file from {}", geoip2_file));
    }

    let engine_data = Box::new(engine_data);
    let engine_data = Box::leak(engine_data);
    let engine = engine_data
        .as_engine()
        .expect("Failed to initialize engine");
    let engine = Box::new(engine);
    let static_engine: &'static Engine<'static> = Box::leak(engine);
    let shared_engine = static_engine;

    let app_state = AppState {
        engine: shared_engine,
        settings: settings.clone(),
    };

    let router = build_router(app_state, &settings);

    let listen_on = format!("{}:{}", settings.host, settings.port);
    #[cfg(feature = "tracing")]
    tracing::info!("Listen on {}", listen_on);

    let listener = tokio::net::TcpListener::bind(&listen_on).await?;
    topcoat::serve(listener, router).await
}

#[cfg(test)]
mod tests;
