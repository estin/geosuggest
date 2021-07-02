use wasm_bindgen::prelude::*;

// wasm-bindgen will automatically take care of including this script
#[wasm_bindgen(module = "/src/map.js")]
extern "C" {
    #[wasm_bindgen(js_name = "mapInit")]
    pub fn map_init(callback: &Closure<dyn FnMut(f64, f64)>);

    #[wasm_bindgen(js_name = "mapMove")]
    pub fn map_move(lat: f64, lng: f64);
}
