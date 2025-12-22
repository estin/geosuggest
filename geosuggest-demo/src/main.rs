use serde::{Deserialize, Serialize};

use reqwasm::http::Request;
use sycamore::futures::{create_resource, spawn_local_scoped};
use sycamore::prelude::*;
use wasm_bindgen::prelude::*;

mod bindings;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CountryItem {
    id: u32,
    code: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminDivisionItem {
    id: u32,
    code: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CityResultItem {
    id: u32,
    name: String,
    country: Option<CountryItem>,
    admin_division: Option<AdminDivisionItem>,
    admin2_division: Option<AdminDivisionItem>,
    timezone: String,
    latitude: f64,
    longitude: f64,
    population: f64,
}

impl CityResultItem {
    pub fn get_country(&self) -> &str {
        if let Some(ref country) = self.country {
            &country.code
        } else {
            ""
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ReverseItem {
    pub city: CityResultItem,
    pub distance: f64,
    pub score: f64,
}

#[derive(Debug)]
pub struct SelectedCity {
    pub city: Option<CityResultItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SuggestQuery<'a> {
    pattern: &'a str,
    limit: Option<usize>,
    /// isolanguage code
    lang: Option<&'a str>,
    min_score: Option<f64>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SuggestResult {
    items: Vec<CityResultItem>,
    /// elapsed time in ms
    time: usize,
}

impl SuggestResult {
    fn new() -> Self {
        SuggestResult {
            items: Vec::new(),
            time: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReverseQuery<'a> {
    lat: f64,
    lng: f64,
    /// isolanguage code
    lang: Option<&'a str>,
    /// population weight
    k: Option<f64>,
}

#[derive(Deserialize, Serialize)]
pub struct ReverseResult {
    items: Vec<ReverseItem>,
    /// elapsed time in ms
    time: usize,
}

enum RequestError {
    #[allow(dead_code)]
    SerializeRequestError(serde_qs::Error),
    #[allow(dead_code)]
    FetchError(reqwasm::Error),
}

impl From<serde_qs::Error> for RequestError {
    fn from(e: serde_qs::Error) -> Self {
        RequestError::SerializeRequestError(e)
    }
}

impl From<reqwasm::Error> for RequestError {
    fn from(e: reqwasm::Error) -> Self {
        RequestError::FetchError(e)
    }
}

fn get_api_url(method: &str) -> String {
    format!(
        "{}{}",
        option_env!("GEOSUGGEST_BASE_API_URL").unwrap_or("http://127.0.0.1:8090"),
        method
    )
}

async fn fetch_suggest(query: SuggestQuery<'_>) -> Result<SuggestResult, RequestError> {
    if query.pattern.is_empty() {
        return Ok(SuggestResult::new());
    }
    let url = get_api_url(&format!(
        "/api/city/suggest?{}",
        serde_qs::to_string(&query)?,
    ));
    let resp = Request::get(&url).send().await?;

    let body = resp.json::<SuggestResult>().await?;
    Ok(body)
}

async fn fetch_reverse(query: ReverseQuery<'_>) -> Result<ReverseResult, RequestError> {
    let url = get_api_url(&format!(
        "/api/city/reverse?{}",
        serde_qs::to_string(&query).unwrap(),
    ));
    let resp = Request::get(&url).send().await?;

    let body = resp.json::<ReverseResult>().await?;
    Ok(body)
}

#[derive(Prop)]
struct SuggestProps<'a> {
    text: &'a ReadSignal<String>,
    lang: &'a ReadSignal<String>,
    min_score: &'a ReadSignal<String>,
}

#[component]
async fn SuggestItems<'a, G: Html>(cx: Scope<'a>, props: SuggestProps<'a>) -> View<G> {
    let selected_item = use_context::<RcSignal<SelectedCity>>(cx);

    let show_suggest = create_selector(cx, move || {
        let text = props.text.get();
        if let Some(city) = &selected_item.get_untracked().city {
            if city.name == text.as_str() {
                return (false, text);
            }
        }
        (true, text)
    });

    let handle_select = move |item: CityResultItem| {
        bindings::map_move(item.latitude, item.longitude);
        selected_item.set(SelectedCity { city: Some(item) });
    };

    let view = create_memo(cx, move || {
        let (show, text) = &*show_suggest.get();

        if !show {
            return view! {cx, };
        }

        if text.is_empty() {
            return view! {cx, };
        }

        let lang = (*props.lang.get()).clone();
        let min_score = (*props.min_score.get()).clone();

        let pattern = create_ref(cx, text.clone());
        let lang = create_ref(cx, lang);
        let min_score = create_ref(cx, min_score);
        let query = SuggestQuery {
            pattern,
            limit: Some(10),
            lang: Some(lang),
            min_score: min_score.parse::<f64>().ok(),
        };
        let items = create_resource(cx, fetch_suggest(query));

        view! {cx,
            div {
                (
                    {
                        if let Some(data) = items.get().as_ref() {
                            if let Ok(d) = data {
                                let views = View::new_fragment(
                                    d.items.iter().map(|item| {
                                        let country = item.get_country().to_owned();
                                        let name = item.name.to_owned();
                                        let item = item.clone();
                                        view! { cx,
                                            li(on:click=move |_| handle_select(item.clone()),class="px-2 py-3 space-x-2 hover:bg-blue-600 hover:text-white focus:bg-blue-600 focus:text-white focus:outline-none"){
                                                (name) " " (country)
                                            }
                                        }
                                    }).collect()
                                );
                                view! {cx,
                                    aside(role="menu",class="absolute z-10 flex flex-col items-start w-64 bg-white border rounded-md shadow-md mt-1") {
                                        ul(class="flex flex-col w-full") {
                                            (views)
                                        }
                                    }
                                }
                            } else {
                                view! {cx, "Error on fetch"}
                            }
                        } else {
                            view! {cx, "loading..."}
                        }
                    }
                )
            }
        }
    });

    view! {cx, div { ((*view.get()).clone()) }}
}

#[component]
async fn ResultView<G: Html>(cx: Scope<'_>) -> View<G> {
    let selected_item = use_context::<RcSignal<SelectedCity>>(cx);
    view! {cx,
        (match selected_item.get().city {
            Some(ref city) => {
                let pretty = serde_json::to_string_pretty(&city).unwrap_or_else(|e| format!("Error: {}", e));

                view! {cx,
                    div(class="w-full px-2 py-1 pb-4") {
                        p(class="font-semibold"){ "City:" }
                        code {
                            pre { (pretty) }
                        }
                    }
                }
            }
            _ => view! {cx, }
        })
    }
}

#[component]
fn App<G: Html>(cx: Scope) -> View<G> {
    // common settings
    let min_score = create_signal(cx, "0.8".to_string());
    let distance_coefficient = create_signal(cx, "0.000000005".to_string());
    let language = create_signal(cx, String::new());

    let suggest_input = create_signal(cx, String::new());
    let reverse_lat = create_signal(cx, String::new());
    let reverse_lng = create_signal(cx, String::new());

    // result city
    let selected_item = create_rc_signal(SelectedCity { city: None });
    let selected_item_clone = selected_item.clone();
    let selected_item_clone2 = selected_item.clone();
    provide_context(cx, selected_item);

    // sync input and selected item
    create_effect(cx, move || {
        let selected = selected_item_clone2.get();
        if let Some(city) = &selected.city {
            suggest_input.set(city.name.clone());
        }
    });

    let do_reverse = Box::new(move || {
        let lat = (*reverse_lat.get_untracked()).clone();
        let lng = (*reverse_lng.get_untracked()).clone();

        if lat.is_empty() || lng.is_empty() {
            return;
        }

        let lang = (*language.get_untracked()).clone();

        let lat = lat.parse::<f64>();
        let lng = lng.parse::<f64>();

        let selected_item_clone = selected_item_clone.clone();
        spawn_local_scoped(cx, async move {
            match (lat, lng) {
                (Ok(lat), Ok(lng)) => {
                    let query = ReverseQuery {
                        lat,
                        lng,
                        lang: Some(&lang),
                        k: None,
                    };
                    if let Ok(result) = fetch_reverse(query).await {
                        if let Some(item) = result.items.first() {
                            selected_item_clone.set(SelectedCity {
                                city: Some(item.city.clone()),
                            });
                        } else {
                            selected_item_clone.set(SelectedCity { city: None });
                        }
                    }
                }
                _ => {
                    log::error!("Invalid lat/lng values");
                }
            };
        });
    });

    // signal to accept coordinates from map events
    let map_click_signal = create_rc_signal((String::new(), String::new()));

    // on map double click set new coordinates
    let map_click_signal_clone = map_click_signal.clone();
    let map_dblclick_closure = Closure::wrap(Box::new(move |lat: f64, lng: f64| {
        log::info!("Map double-click on lat: {} lng: {}", lat, lng);
        map_click_signal_clone.set((lat.to_string(), lng.to_string()));
    }) as Box<dyn FnMut(f64, f64)>);

    // and pass coordinates to manual inputs
    let do_reverse_clone = do_reverse.clone();
    create_effect(cx, move || {
        let c = map_click_signal.get();
        reverse_lat.set(c.0.to_owned());
        reverse_lng.set(c.1.to_owned());
        do_reverse_clone();
    });

    // initialize map
    spawn_local_scoped(cx, async move {
        bindings::map_init(&map_dblclick_closure);
        map_dblclick_closure.forget();
    });

    let handle_reverse = move |_| {
        do_reverse();
    };

    view! { cx,
        div(id="app") {
            div(class="flex h-screen font-sans text-gray-900 bg-gray-300 border-box") {
                div(class="flex flex-row w-full max-w lg:w-1/2 xl:w-1/4 justify-center align-top mb-auto mx-4") {
                    div(class="flex flex-col items-start justify-between h-auto my-4 overflow-hidden bg-white rounded-lg shadow-xl") {
                        div(class="flex flex-row items-baseline justify-around w-full p-1 pt-4 pb-0 mb-0") {
                            h2(class="mr-auto text-lg font-semibold tracking-wide") { "Settings" }
                        }
                        div(class="w-full p-1 pt-0 text-gray-800 bg-gray-100 divide-y divide-gray-400") {
                            div(class="flex flex-col items-center justify-between py-1") {
                                div(class="w-full mt-1") {
                                    label(class="block text-gray-700 text-sm font-bold mb-2",for="min_score") {
                                        "Suggest: Jaro Winkler min score"
                                    }
                                    div(class="mt-1 rounded-md shadow-sm") {
                                        input(bind:value=min_score, id="min_score",type="number",min="0", max="1", class="w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline")
                                    }
                                }
                                div(class="w-full mt-1") {
                                    label(class="block text-gray-700 text-sm font-bold mb-2",for="distance_coefficient") {
                                        "Reverse: Distance correction coefficient by population"
                                    }
                                    div(class="mt-1 rounded-md shadow-sm") {
                                        input(bind:value=distance_coefficient, id="distance_coefficient", type="number", class="w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline")
                                    }
                                }
                            }
                        }
                        div(class="flex flex-row items-baseline justify-around w-full p-1 pt-4 pb-0 mb-0") {
                            h2(class="mr-auto text-lg font-semibold tracking-wide"){ "1. Suggest" }
                        }
                        div(class="w-full p-1 pt-0 text-gray-800 bg-gray-100 divide-y divide-gray-400") {
                            div(class="flex flex-row items-center justify-between py-1") {
                                div(class="w-full") {
                                    div(class="flex") {
                                        div(class="w-5/6") {
                                            div(class="mt-1 flex rounded-md shadow-sm") {
                                                input(bind:value=suggest_input,type="text",placeholder="Please write a city name",class="w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline")
                                            }
                                        }
                                        div(class="ml-1 mt-1 w-1/6 flex rounded-md shadow-sm") {
                                            select(bind:value=language, class="bg-white w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline") {
                                                option(value="en"){"en"}
                                                option(value="ru"){"ru"}
                                                option(value="uk"){"uk"}
                                                option(value="be"){"be"}
                                                option(value="zh"){"zh"}
                                                option(value="ja"){"ja"}
                                          }
                                        }
                                    }
                                    SuggestItems(
                                        text=suggest_input,
                                        lang=language,
                                        min_score=min_score,
                                    )
                                }
                            }
                        }
                        div(class="flex flex-row items-baseline justify-around w-full p-1 pb-0 mb-0") {
                            h2(class="mr-auto text-lg font-semibold tracking-wide"){"2. Reverse (dbl-click on map)"}
                        }
                        div(class="w-full p-1 pt-0 text-gray-800 bg-gray-100 divide-y divide-gray-400") {
                            div(class="flex flex-row items-center justify-between py-1") {
                                div(class="mt-1 w-1/2 pr-1 flex rounded-md shadow-sm") {
                                    // input(on:input=move |event: Event| handle_input("lat", event), value=reverse_lat, placeholder="Latitude", class="w-full px-3 py-1 border border-gray-400 rounded-lg outline-none focus:shadow-outline", type="text")
                                    input(bind:value=reverse_lat, placeholder="Latitude", class="w-full px-3 py-1 border border-gray-400 rounded-lg outline-none focus:shadow-outline", type="text")
                                }
                                div(class="mt-1 w-1/2 flex rounded-md shadow-sm") {
                                    input(bind:value=reverse_lng, placeholder="Longitude", class="w-full px-3 py-1 border border-gray-400 rounded-lg outline-none focus:shadow-outline", type="text")
                                }
                                div(class="mt-1 w-1/3 flex rounded-md shadow-sm") {
                                    button(on:click=handle_reverse, class="w-full ml-1 px-3 py-1 border border-gray-400 rounded-lg outline-none"){"Find"}
                                }
                            }
                        }

                        ResultView { }

                        div(class="flex w-full p-1 mb-1") {
                            h4(class="font-semibold"){"API: "}
                            a(class="mx-1 text-blue-500",href="./swagger"){"Swagger"}
                            " / "
                            a(class="mx-1 text-blue-500",href="./redoc"){"ReDoc"}
                        }
                        div(class="flex w-full p-1 mb-1") {
                            h4(class="font-semibold"){"Github: "}
                            a(class="mx-1 text-blue-500",href="https://github.com/estin/geosuggest"){"geosuggest"}
                        }
                    }
                }
                div(id="map",class="flex-row hidden lg:block lg:w-1/2 xl:w-3/4") {}
            }
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    sycamore::render(|cx| {
        view! {cx,
           App {}
        }
    });
}
