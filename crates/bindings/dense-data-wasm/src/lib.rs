//! WASM bindings for `dense-data`.

use runtime_core::SurfaceRequest;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = NumericSeriesIndex)]
pub struct WasmNumericSeriesIndex {
    inner: dense_data::NumericSeriesIndex,
}

#[wasm_bindgen(js_class = NumericSeriesIndex)]
impl WasmNumericSeriesIndex {
    #[wasm_bindgen(constructor)]
    pub fn new(points: JsValue) -> Result<WasmNumericSeriesIndex, JsValue> {
        let points: Vec<dense_data::NumericSeriesPoint> =
            serde_wasm_bindgen::from_value(points).map_err(into_js_error)?;
        let inner = dense_data::NumericSeriesIndex::from_points(points).map_err(into_js_error)?;
        Ok(Self { inner })
    }

    #[wasm_bindgen(js_name = getSeriesBounds)]
    pub fn get_series_bounds(&self) -> Result<JsValue, JsValue> {
        to_json_value(&self.inner.bounds())
    }

    #[wasm_bindgen(js_name = getBinnedSeries)]
    pub fn get_binned_series(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: dense_data::NumericSeriesBinQuery =
            serde_wasm_bindgen::from_value(query).map_err(into_js_error)?;
        let result = self.inner.bin(query).map_err(into_js_error)?;
        to_json_value(&result)
    }

    #[wasm_bindgen(js_name = getChartSeries)]
    pub fn get_chart_series(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: dense_data::NumericSeriesQuery =
            serde_wasm_bindgen::from_value(query).map_err(into_js_error)?;
        let result = self.inner.get_chart_series(query).map_err(into_js_error)?;
        to_json_value(&result)
    }

    #[wasm_bindgen(js_name = getHistogram)]
    pub fn get_histogram(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: dense_data::NumericHistogramQuery =
            serde_wasm_bindgen::from_value(query).map_err(into_js_error)?;
        let result = self.inner.get_histogram(query).map_err(into_js_error)?;
        to_json_value(&result)
    }

    #[wasm_bindgen(js_name = getHeatmap)]
    pub fn get_heatmap(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: dense_data::NumericHeatmapQuery =
            serde_wasm_bindgen::from_value(query).map_err(into_js_error)?;
        let result = self.inner.get_heatmap(query).map_err(into_js_error)?;
        to_json_value(&result)
    }
}

#[wasm_bindgen(js_name = packageSurface)]
pub fn package_surface() -> Result<JsValue, JsValue> {
    to_json_value(&dense_data::surface::package_surface())
}

#[wasm_bindgen(js_name = runOperation)]
pub fn run_operation(request: JsValue) -> Result<JsValue, JsValue> {
    let request: SurfaceRequest = serde_wasm_bindgen::from_value(request).map_err(into_js_error)?;
    let response = dense_data::surface::run_surface_operation(request).map_err(into_js_error)?;
    to_json_value(&response)
}

fn to_json_value(value: &impl Serialize) -> Result<JsValue, JsValue> {
    let json = serde_json::to_string(value).map_err(into_js_error)?;
    js_sys::JSON::parse(&json)
}

fn into_js_error(error: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn wrapped_surface_has_operations() {
        let surface = dense_data::surface::package_surface();
        assert_eq!(surface.library, "moenarch-dense-data");
        assert!(!surface.operations.is_empty());
    }
}
