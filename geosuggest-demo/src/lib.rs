use geosuggest_core::{Engine, index::ArchivedCitiesRecord};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{Method, PageFn, page},
    runtime::{Event, Signal, shard, signal},
    view::{BoxView, Component, View, view},
};

const STYLE: topcoat::asset::Asset = topcoat::asset::asset!("style.css");
const MAP_SCRIPT: topcoat::asset::Asset = topcoat::asset::asset!("map.js");

pub fn bundle_assets() -> [topcoat::asset::Asset; 3] {
    [STYLE, MAP_SCRIPT, topcoat::runtime::SCRIPT]
}

#[derive(Clone)]
pub struct EngineContext {
    pub engine: &'static Engine<'static>,
    pub url_path_prefix: String,
}

fn route_with_prefix(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        path.to_owned()
    } else if path == "/" {
        format!("/{prefix}")
    } else {
        format!("/{prefix}{path}")
    }
}

type CityView = (u32, String, String, f32, f32, u32, String);

fn city_view(engine: &Engine<'_>, city: &ArchivedCitiesRecord, lang: &str) -> CityView {
    let name = city
        .names
        .as_ref()
        .and_then(|names| names.get(lang))
        .unwrap_or(&city.name)
        .to_string();
    let country = engine
        .city_country(city)
        .map(|country| country.code.as_str().to_owned())
        .unwrap_or_default();

    (
        city.id.to_native(),
        name,
        country,
        city.latitude.to_native(),
        city.longitude.to_native(),
        city.population.to_native(),
        engine.city_timezone(city).to_owned(),
    )
}

#[page("/")]
pub async fn home_page(cx: &Cx) -> Result<impl View> {
    let app = app_context::<EngineContext>(cx);
    let swagger_url = route_with_prefix(&app.url_path_prefix, "/swagger");
    let redoc_url = route_with_prefix(&app.url_path_prefix, "/redoc");
    let query = signal(cx, String::new);
    let language = signal(cx, || "en".to_owned());
    let latitude = signal(cx, String::new);
    let longitude = signal(cx, String::new);
    let min_score = signal(cx, || "0.8".to_owned());
    let distance_coefficient = signal(cx, || "0.000000005".to_owned());
    let selected_city = signal(cx, || 0_u32);

    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"GEOSUGGEST"</title>
                <link rel="stylesheet" href=(STYLE)>
                <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css">
                <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
                <script type="module" src=(MAP_SCRIPT)></script>
                topcoat::runtime::script()
            </head>
            <body>
                <div class="app-layout">
                    <aside class="sidebar">
                        <section class="sidebar-section settings">
                            <h2>"Settings"</h2>
                            <label for="min-score">"Suggest: Jaro Winkler min score"</label>
                            <input id="min-score" type="number" min="0" max="1" step="0.01"
                                :value=$(min_score.get())
                                @input=$(|event: Event| min_score.set(event.target.value))>
                            <label for="distance-coefficient">"Reverse: Distance correction coefficient by population"</label>
                            <input id="distance-coefficient" type="number" step="0.000000001"
                                :value=$(distance_coefficient.get())
                                @input=$(|event: Event| distance_coefficient.set(event.target.value))>
                        </section>
                        <section class="sidebar-section">
                            <div class="section-heading">
                                <h2>"1. Suggest"</h2>
                            </div>
                            <div class="search-row">
                                <label class="visually-hidden" for="city-query">"City name"</label>
                                <input id="city-query" type="search" placeholder="Please write a city name"
                                    :value=$(query.get())
                                    @input=$(|event: Event| query.set(event.target.value))>
                                <label class="visually-hidden" for="language">"Language"</label>
                                <select id="language" :value=$(language.get())
                                    @change=$(|event: Event| language.set(event.target.value))>
                                    <option value="en">"en"</option>
                                    <option value="ru">"ru"</option>
                                    <option value="uk">"uk"</option>
                                    <option value="be">"be"</option>
                                    <option value="zh">"zh"</option>
                                    <option value="ja">"ja"</option>
                                </select>
                            </div>
                            suggest_results(
                                pattern: $(query.get()),
                                lang: $(language.get()),
                                min_score: $(min_score.get()),
                                selected_city: selected_city.clone()
                            )
                        </section>
                        <section class="sidebar-section reverse-section">
                            <div class="section-heading">
                                <h2>"2. Reverse (double-click on map)"</h2>
                            </div>
                            <div class="coordinates">
                                <label class="visually-hidden" for="latitude">"Latitude"</label>
                                <input id="latitude" inputmode="decimal" placeholder="Latitude"
                                    :value=$(latitude.get())
                                    @input=$(|event: Event| latitude.set(event.target.value))>
                                <label class="visually-hidden" for="longitude">"Longitude"</label>
                                <input id="longitude" inputmode="decimal" placeholder="Longitude"
                                    :value=$(longitude.get())
                                    @input=$(|event: Event| longitude.set(event.target.value))>
                                <button type="button" class="find-button" data-find-city="true">"Find"</button>
                            </div>
                            reverse_results(
                                lat: $(latitude.get()),
                                lng: $(longitude.get()),
                                lang: $(language.get()),
                                coefficient: $(distance_coefficient.get()),
                                selected_city: selected_city.clone()
                            )
                        </section>
                        <section class="sidebar-section city-section">
                            <h2>"City:"</h2>
                            city_detail(city_id: $(selected_city.get()), lang: $(language.get()))
                        </section>
                        <footer class="sidebar-footer">
                            <div>
                                <strong>"API:"</strong>
                                <a href=(swagger_url)>"Swagger"</a>
                                <span>"/"</span>
                                <a href=(redoc_url)>"ReDoc"</a>
                            </div>
                            <div>
                                <strong>"GitHub:"</strong>
                                <a href="https://github.com/estin/geosuggest">"geosuggest"</a>
                            </div>
                        </footer>
                    </aside>
                    <main class="map-panel">
                        <div id="map" role="application" aria-label="City map"></div>
                    </main>
                </div>
            </body>
        </html>
    })
}

#[shard("/_geosuggest/suggest")]
pub async fn suggest_results(
    cx: &Cx,
    pattern: String,
    lang: String,
    min_score: String,
    selected_city: Signal<u32>,
) -> Result<impl View> {
    let pattern = pattern.trim();
    let parsed_score = min_score.parse::<f32>();
    let message = if pattern.is_empty() {
        "Search results will appear here."
    } else if pattern.len() > 128 {
        "Search text is too long."
    } else if !matches!(parsed_score, Ok(score) if score.is_finite() && (0.0..=1.0).contains(&score))
    {
        "Enter a score from 0 to 1."
    } else {
        ""
    };
    let results = if message.is_empty() {
        let engine = app_context::<EngineContext>(cx).engine;
        engine
            .suggest(pattern, 10, parsed_score.ok(), None::<&[&str]>)
            .into_iter()
            .map(|city| city_view(engine, city, &lang))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    Ok(view! {
        if !message.is_empty() {
            <p class="hint">(message)</p>
        } else if results.is_empty() {
            <p class="hint">"No matching cities."</p>
        } else {
            <ul class="results" aria-live="polite">
                for (id, name, country, latitude, longitude, population, timezone) in results {
                    <li class="result">
                        <button type="button" class="city-result" data-map-city="true"
                            data-lat=(latitude) data-lng=(longitude)
                            @click=$(|_event: Event| selected_city.set(id))>
                            <strong>(name)</strong>
                            <span class="country">(country)</span>
                            <span class="details">"ID " (id) " · " (latitude) ", " (longitude)
                                " · population " (population) " · " (timezone)</span>
                        </button>
                    </li>
                }
            </ul>
        }
    })
}

#[shard("/_geosuggest/reverse")]
pub async fn reverse_results(
    cx: &Cx,
    lat: String,
    lng: String,
    lang: String,
    coefficient: String,
    selected_city: Signal<u32>,
) -> Result<impl View> {
    let (mut message, coordinates) = if lat.is_empty() || lng.is_empty() {
        ("Nearby cities will appear here.", None)
    } else {
        match (lat.parse::<f32>(), lng.parse::<f32>()) {
            (Ok(lat), Ok(lng))
                if lat.is_finite()
                    && (-90.0..=90.0).contains(&lat)
                    && lng.is_finite()
                    && (-180.0..=180.0).contains(&lng) =>
            {
                ("", Some((lat, lng)))
            }
            (Ok(_), Ok(_)) => ("Coordinates are outside the valid range.", None),
            _ => ("Enter valid coordinates.", None),
        }
    };
    let parsed_coefficient = coefficient.parse::<f32>();
    if message.is_empty()
        && !matches!(parsed_coefficient, Ok(value) if value.is_finite() && value >= 0.0)
    {
        message = "Enter a valid population correction value.";
    }
    let results = if message.is_empty() {
        let (lat, lng) = coordinates.expect("valid coordinates are present without an error");
        let engine = app_context::<EngineContext>(cx).engine;
        engine
            .reverse((lat, lng), 10, parsed_coefficient.ok(), None::<&[&str]>)
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                let (id, name, country, latitude, longitude, population, timezone) =
                    city_view(engine, item.city, &lang);
                (
                    id,
                    name,
                    country,
                    latitude,
                    longitude,
                    population,
                    timezone,
                    item.distance,
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    Ok(view! {
        if !message.is_empty() {
            <p class="hint">(message)</p>
        } else if results.is_empty() {
            <p class="hint">"No nearby cities were found."</p>
        } else {
            <ul class="results" aria-live="polite">
                for (id, name, country, latitude, longitude, population, timezone, distance) in results {
                    <li class="result">
                        <button type="button" class="city-result" data-map-city="true"
                            data-lat=(latitude) data-lng=(longitude)
                            @click=$(|_event: Event| selected_city.set(id))>
                            <strong>(name)</strong>
                            <span class="country">(country)</span>
                            <span class="details">"ID " (id) " · " (distance) " km away · " (latitude)
                                ", " (longitude) " · population " (population) " · " (timezone)</span>
                        </button>
                    </li>
                }
            </ul>
        }
    })
}

#[shard("/_geosuggest/city")]
pub async fn city_detail(cx: &Cx, city_id: u32, lang: String) -> Result<impl View> {
    let engine = app_context::<EngineContext>(cx).engine;
    let city = engine
        .get(&city_id)
        .map(|city| city_view(engine, city, &lang));

    Ok(view! {
        if let Some((id, name, country, latitude, longitude, population, timezone)) = city {
            <dl class="city-details">
                <div><dt>"Name"</dt><dd>(name)</dd></div>
                <div><dt>"Geoname ID"</dt><dd>(id)</dd></div>
                <div><dt>"Country"</dt><dd>(country)</dd></div>
                <div><dt>"Coordinates"</dt><dd>(latitude) ", " (longitude)</dd></div>
                <div><dt>"Population"</dt><dd>(population)</dd></div>
                <div><dt>"Timezone"</dt><dd>(timezone)</dd></div>
            </dl>
        } else {
            <p class="hint">"Select a city result to view its details."</p>
        }
    })
}

pub fn home_page_at(path: String) -> PageFn {
    let path: &'static str = Box::leak(path.into_boxed_str());
    PageFn::new(Method::GET, path, render_home_page)
}

fn render_home_page<'a>(cx: &'a Cx, body: topcoat::router::Body) -> BoxView<'a> {
    Box::pin(topcoat::view::HoistView::new(
        topcoat::view::internal::ThenView::new(async move {
            drop(body);
            let props = <home_page as Component>::props_builder().build();
            <home_page as Component>::render(home_page, cx, props).await
        }),
    ))
}
