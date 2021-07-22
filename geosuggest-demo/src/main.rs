use serde::{Deserialize, Serialize};

use wasm_bindgen::prelude::*;
use yew::format::{Json, Nothing};
use yew::services::fetch::{
    Credentials, FetchOptions, FetchService, FetchTask, Mode, Request, Response,
};
use yew::services::ConsoleService;
use yew::{html, ChangeData, Component, ComponentLink, Html, InputData, ShouldRender};

mod bindings;

#[derive(Serialize, Deserialize)]
pub struct CityResultItem {
    id: usize,
    name: String,
    country_code: String,
    timezone: String,
    latitude: f64,
    longitude: f64,
    population: f64,
}

#[derive(Serialize, Deserialize)]
pub struct ReverseItem {
    pub city: CityResultItem,
    pub distance: f64,
    pub score: f64,
}

pub enum Msg {
    FetchResourceFailed,
    SuggestInput(String),
    SuggestResult(SuggestResult),
    SuggestItemSelected(usize),
    LanguageSelected(ChangeData),
    ReverseLatInput(String),
    ReverseLngInput(String),
    ReverseFind,
    ReverseResult(ReverseResult),
    MapDblClick(f64, f64),
    MinScoreInput(String),
    DistanceCoefficient(String),
}

pub struct Model {
    link: ComponentLink<Self>,
    suggest_selected_item: Option<usize>,
    suggest_items: Option<Vec<CityResultItem>>,
    lang: Option<String>,
    reverse_lng: Option<f64>,
    reverse_lat: Option<f64>,
    _ft: Option<FetchTask>,
    reverse_result: Option<ReverseItem>,
    loading: bool,
    map_dblclick_closure: Closure<dyn FnMut(f64, f64)>,
    min_score: String,
    distance_coefficient: String,
}

#[inline]
fn get_api_url(method: &str) -> String {
    format!(
        "{}{}",
        option_env!("GEOSUGGEST_BASE_API_URL").unwrap_or("http://127.0.0.1:8090"),
        method
    )
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SuggestQuery<'a> {
    pattern: &'a str,
    limit: Option<usize>,
    /// isolanguage code
    lang: Option<&'a str>,
    min_score: Option<f64>,
}

#[derive(Deserialize, Serialize)]
pub struct SuggestResult {
    items: Vec<CityResultItem>,
    /// elapsed time in ms
    time: usize,
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

impl Model {
    fn suggest(&mut self, text: &str) {
        if text.is_empty() {
            self.suggest_items = None;
            self.suggest_selected_item = None;
            return;
        }

        self.loading = true;
        let query = SuggestQuery {
            pattern: text,
            limit: Some(5),
            lang: self.lang.as_deref(),
            min_score: self.min_score.parse().ok(),
        };
        let request = Request::get(get_api_url(&format!(
            "/api/city/suggest?{}",
            serde_qs::to_string(&query).unwrap(),
        )))
        .header("Access-Control-Request-Method", "GET")
        .body(Nothing)
        .expect("Failed to build request.");

        let callback = self.link.callback(
            |response: Response<Json<Result<SuggestResult, anyhow::Error>>>| {
                if let (meta, Json(Ok(body))) = response.into_parts() {
                    if meta.status.is_success() {
                        log::info!("Data: {:?}", serde_json::to_string(&body));
                        // return Msg::SuggestResult(serde_json::to_string(&body).unwrap());
                        return Msg::SuggestResult(body);
                    }
                }
                Msg::FetchResourceFailed
            },
        );

        self._ft = FetchService::fetch_with_options(
            request,
            FetchOptions {
                mode: Some(Mode::Cors),
                credentials: Some(Credentials::SameOrigin),
                ..FetchOptions::default()
            },
            callback,
        )
        .ok();
    }
    fn reverse(&mut self) {
        match (self.reverse_lat, self.reverse_lng) {
            (Some(lat), Some(lng)) => {
                ConsoleService::log(&format!("Rerverse find {} {}", lat, lng));
                self.loading = true;
                let query = ReverseQuery {
                    lat,
                    lng,
                    lang: self.lang.as_deref(),
                    k: self.distance_coefficient.parse().ok(),
                };
                let request = Request::get(get_api_url(&format!(
                    "/api/city/reverse?{}",
                    serde_qs::to_string(&query).unwrap(),
                )))
                .header("Access-Control-Request-Method", "GET")
                .body(Nothing)
                .expect("Failed to build request.");

                let callback = self.link.callback(
                    |response: Response<Json<Result<ReverseResult, anyhow::Error>>>| {
                        if let (meta, Json(Ok(body))) = response.into_parts() {
                            if meta.status.is_success() {
                                ConsoleService::log(&format!(
                                    "Data: {:?}",
                                    serde_json::to_string(&body)
                                ));
                                return Msg::ReverseResult(body);
                            }
                        }
                        Msg::FetchResourceFailed
                    },
                );

                self._ft = FetchService::fetch_with_options(
                    request,
                    FetchOptions {
                        mode: Some(Mode::Cors),
                        credentials: Some(Credentials::SameOrigin),
                        ..FetchOptions::default()
                    },
                    callback,
                )
                .ok();
            }
            _ => {
                ConsoleService::log(&"not valid reverse input data".to_string());
            }
        }
    }
}

// fn tooltip(text: &str) -> Html {
//     html! {
//         <div class="relative flex flex-col items-center group inline-block">
//            <svg class="w-5 h-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
//                <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-3a1 1 0 00-.867.5 1 1 0 11-1.731-1A3 3 0 0113 8a3.001 3.001 0 01-2 2.83V11a1 1 0 11-2 0v-1a1 1 0 011-1 1 1 0 100-2zm0 8a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd" />
//            </svg>
//            <div class="absolute bottom-0 flex flex-col items-center hidden mb-6 group-hover:flex">
//                <span class="relative z-10 p-2 text-xs leading-none text-white whitespace-no-wrap bg-black shadow-lg">{text}</span>
//                <div class="w-3 h-3 -mt-2 rotate-45 bg-black"></div>
//            </div>
//        </div>
//     }
// }

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let link_clone = link.clone();

        Self {
            link,
            _ft: None,
            suggest_items: None,
            suggest_selected_item: None,
            loading: false,
            lang: None,
            reverse_lat: None,
            reverse_lng: None,
            reverse_result: None,
            min_score: 0.8.to_string(),
            distance_coefficient: 0.000000005.to_string(),
            map_dblclick_closure: Closure::wrap(Box::new(move |lat: f64, lng: f64| {
                ConsoleService::log(&format!("map doubl click {} {}", lat, lng));
                link_clone.send_message(Msg::MapDblClick(lat, lng));
            }) as Box<dyn FnMut(f64, f64)>),
        }
    }

    fn rendered(&mut self, first_render: bool) {
        if first_render {
            bindings::map_init(&self.map_dblclick_closure);
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SuggestInput(v) => {
                log::info!("Suggest input is {}", v);
                self.suggest_items = None;
                self.reverse_result = None;
                self.suggest(v.as_str());
                true
            }
            Msg::SuggestResult(result) => {
                self.loading = false;
                self.reverse_result = None;
                self.suggest_selected_item = None;
                self.suggest_items = Some(result.items);
                true
            }
            Msg::SuggestItemSelected(index) => {
                self.suggest_selected_item = Some(index);
                if let Some(items) = &self.suggest_items {
                    if let Some(item) = items.get(index) {
                        bindings::map_move(item.latitude, item.longitude);
                    }
                }
                true
            }
            Msg::LanguageSelected(ChangeData::Select(lang)) => {
                self.lang = Some(lang.value());
                false
            }
            Msg::ReverseLatInput(value) => {
                ConsoleService::log(&format!("Lat input {}", value));
                if let Ok(lat) = value.parse() {
                    self.reverse_lat = Some(lat);
                } else {
                    ConsoleService::log(&format!("Lat invalid input {}", value));
                }
                false
            }
            Msg::ReverseLngInput(value) => {
                ConsoleService::log(&format!("Lng input {}", value));
                if let Ok(lng) = value.parse() {
                    self.reverse_lng = Some(lng);
                } else {
                    ConsoleService::log(&format!("Lng invalid input {}", value));
                }
                false
            }
            Msg::ReverseFind => {
                ConsoleService::log(&format!(
                    "Reverse {:?} {:?}",
                    self.reverse_lat, self.reverse_lng
                ));
                self.reverse_result = None;
                self.suggest_items = None;
                self.suggest_selected_item = None;
                self.reverse();
                true
            }
            Msg::ReverseResult(result) => {
                self.loading = false;
                self.reverse_result = result.items.into_iter().next();
                true
            }
            Msg::MapDblClick(lat, lng) => {
                self.reverse_lat = Some(lat);
                self.reverse_lng = Some(lng);
                self.link.send_message(Msg::ReverseFind);
                true
            }
            Msg::MinScoreInput(value) => {
                ConsoleService::log(&format!("MinScoreInput input {}", value));
                self.min_score = value;
                true
            }
            Msg::DistanceCoefficient(value) => {
                ConsoleService::log(&format!("DistanceCoefficient input {}", value));
                self.distance_coefficient = value;
                true
            }

            _ => false,
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        let suggest_items = match &self.suggest_items {
            Some(items) => {
                if self.suggest_selected_item.is_some() {
                    html! {}
                } else {
                    html! {
                        <aside role="menu" class="absolute z-10 flex flex-col items-start w-64 bg-white border rounded-md shadow-md mt-1">
                            <ul class="flex flex-col w-full">
                            { items.iter().enumerate().map(|(index, item)| html! {
                                <li onclick=self.link.callback(move |_| Msg::SuggestItemSelected(index)) class="px-2 py-3 space-x-2 hover:bg-blue-600 hover:text-white focus:bg-blue-600 focus:text-white focus:outline-none ">{ &item.name } {" "} { &item.country_code }</li>
                            }).collect::<Html>()}
                            </ul>
                        </aside>
                    }
                }
            }
            None => {
                if self.loading {
                    html! { {"Loading"} }
                } else {
                    html! {}
                }
            }
        };

        let result_node = match (
            &self.reverse_result,
            self.suggest_selected_item,
            &self.suggest_items,
        ) {
            (Some(item), _, _) => {
                let pretty = serde_json::to_string_pretty(&item).unwrap();
                html! { <div class="w-full px-2 py-1 pb-4"><p class="font-semibold">{ "Reverse result:" }</p><code><pre>{ pretty }</pre></code></div> }
            }
            (None, Some(index), Some(items)) => {
                let pretty = serde_json::to_string_pretty(&items[index]).unwrap();
                html! { <div class="w-full px-2 py-1 pb-4"><p class="font-semibold">{ "Suggest result:" }</p><code><pre>{ pretty }</pre></code></div> }
            }
            _ => html! {},
        };

        let lat = self.reverse_lat.map(|v| format!("{:.5}", v));
        let lng = self.reverse_lng.map(|v| format!("{:.5}", v));

        html! {
            <div id="app">
                <div class="flex h-screen font-sans text-gray-900 bg-gray-300 border-box">
                    <div class="flex flex-row w-full max-w lg:w-1/2 xl:w-1/4 justify-center align-top mb-auto mx-4">
                        <div class="flex flex-col items-start justify-between h-auto my-4 overflow-hidden bg-white rounded-lg shadow-xl">
                            <div class="flex flex-row items-baseline justify-around w-full p-1 pt-4 pb-0 mb-0">
                                <h2 class="mr-auto text-lg font-semibold tracking-wide">{ "Settings" }</h2>
                            </div>
                            <div class="w-full p-1 pt-0 text-gray-800 bg-gray-100 divide-y divide-gray-400">
                                <div class="flex flex-col items-center justify-between py-1">
                                    <div class="w-full mt-1">
                                        <label class="block text-gray-700 text-sm font-bold mb-2" for="min_score">
                                            {"Suggest: Jaro Winkler min score"}
                                        </label>
                                        <div class="mt-1 rounded-md shadow-sm">
                                            <input id="min_score" value=self.min_score.clone() type="number" min="0" max="1" oninput=self.link.callback(|event: InputData| Msg::MinScoreInput(event.value)) class="w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline" />
                                        </div>
                                    </div>
                                    <div class="w-full mt-1">
                                        <label class="block text-gray-700 text-sm font-bold mb-2" for="distance_coefficient">
                                            {"Reverse: Distance correction coefficient by population"}
                                        </label>
                                        <div class="mt-1 rounded-md shadow-sm">
                                            <input id="distance_coefficient" value=self.distance_coefficient.clone() type="number" oninput=self.link.callback(|event: InputData| Msg::DistanceCoefficient(event.value)) class="w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline" />
                                        </div>
                                    </div>
                                </div>
                            </div>
                            <div class="flex flex-row items-baseline justify-around w-full p-1 pt-4 pb-0 mb-0">
                                <h2 class="mr-auto text-lg font-semibold tracking-wide">{ "1. Suggest" }</h2>
                            </div>
                            <div class="w-full p-1 pt-0 text-gray-800 bg-gray-100 divide-y divide-gray-400">
                                <div class="flex flex-row items-center justify-between py-1">
                                    <div class="w-full">
                                        <div class="flex">
                                            <div class="w-5/6">
                                                <div class="mt-1 flex rounded-md shadow-sm">
                                                    <input oninput=self.link.callback(|event: InputData| Msg::SuggestInput(event.value)) type="text" placeholder="Please write a city name" class="w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline" />
                                                </div>
                                            </div>
                                            <div class="ml-1 mt-1 w-1/6 flex rounded-md shadow-sm">
                                                <select onchange=self.link.callback(Msg::LanguageSelected) class="bg-white w-full px-3 py-2 border border-gray-400 rounded-lg outline-none focus:shadow-outline" name="whatever" id="frm-whatever">
                                                    <option value="en">{"en"}</option>
                                                    <option value="ru">{"ru"}</option>
                                                    <option value="uk">{"uk"}</option>
                                                    <option value="be">{"be"}</option>
                                                    <option value="zh">{"zh"}</option>
                                                    <option value="ja">{"ja"}</option>
                                                </select>
                                            </div>
                                        </div>
                                        { suggest_items }
                                    </div>
                                </div>
                            </div>
                            <div class="flex flex-row items-baseline justify-around w-full p-1 pb-0 mb-0">
                                <h2 class="mr-auto text-lg font-semibold tracking-wide">{"2. Reverse (dbl-click on map)"}</h2>
                            </div>
                            <div class="w-full p-1 pt-0 text-gray-800 bg-gray-100 divide-y divide-gray-400">
                                <div class="flex flex-row items-center justify-between py-1">
                                    <div class="mt-1 w-1/2 pr-1 flex rounded-md shadow-sm">
                                        <input oninput=self.link.callback(|event: InputData| Msg::ReverseLatInput(event.value)) value=lat placeholder="Latitude" class="w-full px-3 py-1 border border-gray-400 rounded-lg outline-none focus:shadow-outline" type="text" />
                                    </div>
                                    <div class="mt-1 w-1/2 flex rounded-md shadow-sm">
                                        <input oninput=self.link.callback(|event: InputData| Msg::ReverseLngInput(event.value)) value=lng placeholder="Longitude" class="w-full px-3 py-1 border border-gray-400 rounded-lg outline-none focus:shadow-outline" type="text" />
                                    </div>
                                    <div class="mt-1 w-1/3 flex rounded-md shadow-sm">
                                        <button onclick=self.link.callback(move |_| Msg::ReverseFind) class="w-full ml-1 px-3 py-1 border border-gray-400 rounded-lg outline-none">{"Find"}</button>
                                    </div>
                                </div>
                            </div>
                            { result_node }
                            <hr/>
                            <div class="flex w-full p-1 mb-1">
                                <h4 class="font-semibold">{"API: "}</h4>
                                <a class="mx-1 text-blue-500" href="/swagger">{"Swagger"}</a>
                                { " / " }
                                <a class="mx-1 text-blue-500" href="/redoc">{"ReDoc"}</a>
                            </div>
                        </div>
                    </div>
                    <div id="map" class="flex-row hidden lg:block lg:w-1/2 xl:w-3/4"></div>
                </div>
            </div>
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
