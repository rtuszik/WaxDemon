use leptos::prelude::*;
use serde_json::{Value, json};

pub fn history_options(history: &[Value], currency: &str, counts: bool) -> Value {
    let points = history
        .iter()
        .filter(|v| counts || v["currency"].as_str().unwrap_or("unknown") == currency)
        .count();
    let series: Vec<_> = if counts {
        vec![
            json!({"id":"records","name":"Records","type":"line","showSymbol":points==1,"data":history.iter().map(|v|json!([v["timestamp"],v["total_items"]])).collect::<Vec<_>>()}),
        ]
    } else {
        [("minimum", "Minimum"), ("median", "Median"), ("maximum", "Maximum")]
            .into_iter().map(|(field, label)| json!({
                "id":field,"name":label,"type":"line","showSymbol":points==1,"connectNulls":false,"smooth":true,"lineStyle":{"width":2},
                "data":history.iter().filter(|v|v["currency"].as_str().unwrap_or("unknown")==currency).map(|v| {
                    let amount=v[field].as_str().and_then(|s|s.parse::<f64>().ok()).filter(|n|n.is_finite());
                    json!([v["timestamp"],amount])
                }).collect::<Vec<_>>()
            })).collect()
    };
    json!({"animation":false,"aria":{"enabled":true},"backgroundColor":"transparent","textStyle":{"fontFamily":"system-ui, -apple-system, Segoe UI, Roboto, sans-serif","color":"#a3a3a3"},"color":["#3b82f6","#a855f7","#22c55e"],
        "tooltip":{"trigger":"axis","renderMode":"richText","backgroundColor":"#171717","borderColor":"#262626","textStyle":{"color":"#d4d4d4"}},"legend":{"top":0,"textStyle":{"color":"#d4d4d4"}},
        "grid":{"left":65,"right":30,"top":60,"bottom":80},
        "xAxis":{"type":"time","axisLabel":{"color":"#a3a3a3"},"axisLine":{"lineStyle":{"color":"#262626"}}},"yAxis":{"type":"value","axisLabel":{"color":"#a3a3a3"},"splitLine":{"lineStyle":{"color":"#262626","type":"dashed"}},"name":if counts {"Records"} else if currency=="unknown" {"Unknown currency"} else {currency}},
        "dataZoom":[{"type":"inside","filterMode":"none"},{"type":"slider","bottom":8,"borderColor":"#262626","backgroundColor":"#171717","fillerColor":"rgba(59,130,246,0.15)","textStyle":{"color":"#a3a3a3"}}],"series":series})
}

pub fn breakdown_options(values: &[Value]) -> Value {
    json!({"animation":false,"aria":{"enabled":true},"backgroundColor":"transparent","textStyle":{"fontFamily":"system-ui, -apple-system, Segoe UI, Roboto, sans-serif","color":"#a3a3a3"},"color":["#3b82f6","#a855f7","#10b981","#ec4899","#f97316","#06b6d4","#f59e0b","#6366f1","#84cc16","#d946ef","#737373"],
        "tooltip":{"trigger":"item","renderMode":"richText","backgroundColor":"#171717","borderColor":"#262626","textStyle":{"color":"#d4d4d4"}},
        "series":[{"id":"breakdown","type":"pie","radius":["42%","68%"],"label":{"show":false},"itemStyle":{"borderColor":"#171717","borderWidth":2},
            "data":values.iter().map(|v|json!({"name":v["name"],"value":v["count"]})).collect::<Vec<_>>()}]})
}

#[component]
pub fn Chart(
    options: Signal<Value>,
    label: &'static str,
    #[prop(optional)] filter: Option<&'static str>,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Div>::new();
    let error = RwSignal::new(false);
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::{JsCast, prelude::*};
        let mounted = StoredValue::new_local(None::<browser::Mounted>);
        let navigate = leptos_router::hooks::use_navigate();
        Effect::new(move |_| {
            let Some(element) = node.get() else {
                return;
            };
            let mut option = options.get();
            mounted.update_value(|state| {
                if state.is_none() {
                    let Ok(chart) = browser::init(
                        element.as_ref(),
                        &JsValue::from_str("dark"),
                        &browser::value(&json!({"renderer":"canvas"})),
                    ) else {
                        error.set(true);
                        return;
                    };
                    let resize_chart = chart.clone();
                    let resize = Closure::<dyn FnMut()>::new(move || {
                        let _ = resize_chart.resize();
                    });
                    let observer =
                        web_sys::ResizeObserver::new(resize.as_ref().unchecked_ref()).ok();
                    if let Some(observer) = &observer {
                        observer.observe(element.as_ref());
                    }
                    let navigate = navigate.clone();
                    let click = Closure::<dyn FnMut(JsValue)>::new(move |event| {
                        if let Some(filter) = filter
                            && let Ok(name) = js_sys::Reflect::get(&event, &"name".into())
                            && let Some(name) = name.as_string()
                            && !(filter == "year" && name == "Unknown")
                        {
                            let query = url::form_urlencoded::Serializer::new(String::new())
                                .append_pair(filter, &name)
                                .finish();
                            navigate(&format!("/library?{query}"), Default::default());
                        }
                    });
                    let _ = chart.on("click", click.as_ref().unchecked_ref());
                    *state = Some(browser::Mounted {
                        chart,
                        observer,
                        _resize: resize,
                        _click: click,
                        initialized: false,
                    });
                }
                if let Some(state) = state {
                    if state.initialized {
                        option.as_object_mut().unwrap().remove("dataZoom");
                    }
                    let value = browser::value(&option);
                    let settings = browser::value(&json!({"replaceMerge":["series"]}));
                    error.set(state.chart.set_option(&value, &settings).is_err());
                    state.initialized = true;
                }
            });
        });
        on_cleanup(move || {
            mounted.update_value(|state| {
                if let Some(state) = state.take() {
                    if let Some(observer) = state.observer {
                        observer.disconnect();
                    }
                    let _ = state.chart.dispose();
                }
            })
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (options, filter);
    view! {<div node_ref=node class="chart" role="img" aria-label=label data-chart=label></div>{move ||error.get().then(||view!{<p class="error">"Chart could not be displayed. The data is available below."</p>})}}
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use serde::Serialize;
    use wasm_bindgen::prelude::*;
    pub fn value(value: &serde_json::Value) -> JsValue {
        value
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .unwrap()
    }
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace=echarts,catch)]
        pub fn init(
            element: &web_sys::Element,
            theme: &JsValue,
            options: &JsValue,
        ) -> Result<Instance, JsValue>;
        #[derive(Clone)]
        pub type Instance;
        #[wasm_bindgen(method,catch,js_name=setOption)]
        pub fn set_option(
            this: &Instance,
            options: &JsValue,
            settings: &JsValue,
        ) -> Result<(), JsValue>;
        #[wasm_bindgen(method, catch)]
        pub fn resize(this: &Instance) -> Result<(), JsValue>;
        #[wasm_bindgen(method, catch)]
        pub fn dispose(this: &Instance) -> Result<(), JsValue>;
        #[wasm_bindgen(method, catch)]
        pub fn on(this: &Instance, event: &str, callback: &js_sys::Function)
        -> Result<(), JsValue>;
    }
    pub struct Mounted {
        pub chart: Instance,
        pub observer: Option<web_sys::ResizeObserver>,
        pub _resize: Closure<dyn FnMut()>,
        pub _click: Closure<dyn FnMut(JsValue)>,
        pub initialized: bool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valuation_never_combines_currencies_or_turns_missing_prices_into_zero() {
        let history = vec![
            json!({"timestamp":"2025-01-01","currency":"EUR","median":"12.345","total_items":2}),
            json!({"timestamp":"2025-01-02","currency":"USD","median":"900","total_items":3}),
            json!({"timestamp":"2025-01-03","currency":null,"median":"40","total_items":4}),
        ];
        let eur = history_options(&history, "EUR", false);
        assert_eq!(eur["series"][1]["data"], json!([["2025-01-01", 12.345]]));
        assert_eq!(eur["series"][0]["data"], json!([["2025-01-01", null]]));
        assert_eq!(eur["yAxis"]["name"], "EUR");
        assert_eq!(eur["series"][1]["showSymbol"], true);
        assert_eq!(
            history_options(&history, "EUR", true)["series"][0]["showSymbol"],
            false
        );
        assert_eq!(
            history_options(&history, "unknown", false)["series"][1]["data"],
            json!([["2025-01-03", 40.0]])
        );
        assert_eq!(
            history_options(&history, "EUR", true)["series"][0]["data"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn breakdown_preserves_names_and_counts_for_filtering() {
        let options = breakdown_options(&[json!({"name":"Jazz & Blues","count":8})]);
        assert_eq!(
            options["series"][0]["data"],
            json!([{"name":"Jazz & Blues","value":8}])
        );
        assert_eq!(options["tooltip"]["renderMode"], "richText");
    }
}
