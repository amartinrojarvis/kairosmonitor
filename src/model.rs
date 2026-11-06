use crate::config::Config;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Kairos,
    OpenAI,
}
#[derive(Debug, Clone)]
pub struct Sample {
    pub data: Value,
    pub tps: f64,
    pub kind: ServerKind,
    pub models: Vec<String>,
    pub selected_service: String,
    pub endpoint: String,
}
pub fn number(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .filter(|n| n.is_finite())
        .unwrap_or(-1.)
}
pub fn string(v: &Value) -> String {
    v.as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("--")
        .to_owned()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertLevel {
    Normal,
    Warning,
    Critical,
}
impl AlertLevel {
    pub fn for_value(value: f64, warn: f64, critical: f64) -> Self {
        if !value.is_finite() || value < 0. {
            Self::Normal
        } else if value >= critical {
            Self::Critical
        } else if value >= warn {
            Self::Warning
        } else {
            Self::Normal
        }
    }
}
impl Sample {
    pub fn parse(raw: &[u8], service: &str) -> Result<Self, String> {
        let data: Value = serde_json::from_slice(raw).map_err(|_| "JSON inválido".to_string())?;
        if !data.is_object() {
            return Err("Respuesta inesperada".into());
        }
        let tp = &data["vllmThroughput"];
        let mut tps = number(&tp["tps"]);
        let models = discover_kairos_models(&data);
        let selected_service = if service == "auto" {
            data["services"]
                .as_array()
                .and_then(|items| {
                    items
                        .iter()
                        .find(|v| v["model"].is_string() && v["model"] == tp["model"])
                        .or_else(|| {
                            items.iter().find(|v| {
                                v["running"] != false
                                    && v["model"]
                                        .as_str()
                                        .is_some_and(|m| models.iter().any(|known| known == m))
                            })
                        })
                })
                .and_then(|v| v["id"].as_str())
                .unwrap_or("")
                .to_owned()
        } else {
            service.to_owned()
        };
        let svc = data["services"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == selected_service));
        if tp["ready"] == false || tp["ok"] == false || svc.is_some_and(|s| s["running"] == false) {
            tps = -1.;
        }
        if number(&data["memory"]["percent"]) < 0.
            && number(&data["gpu"][0]["utilization"]) < 0.
            && tps < 0.
        {
            return Err("Sin métricas en la respuesta".into());
        }
        Ok(Self {
            data,
            tps,
            kind: ServerKind::Kairos,
            models,
            selected_service,
            endpoint: String::new(),
        })
    }
    pub fn from_openai(raw: &[u8], host: &str) -> Result<Self, String> {
        let catalog: Value =
            serde_json::from_slice(raw).map_err(|_| "JSON inválido".to_string())?;
        let entries = catalog
            .get("data")
            .and_then(Value::as_array)
            .ok_or("El servidor no devuelve un catálogo de modelos compatible con OpenAI")?;
        if catalog["object"] != "list" && entries.is_empty() {
            return Err("Catálogo de modelos no válido".into());
        }
        let mut models = Vec::new();
        for entry in entries {
            let name = entry["id"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or("Catálogo con un modelo sin identificador")?;
            if !models.iter().any(|m| m == name) {
                models.push(name.to_owned());
            }
        }
        let data = serde_json::json!({"hostname":host,"vllmThroughput":{"model":models.first().cloned().unwrap_or_default()}});
        Ok(Self {
            data,
            tps: -1.,
            kind: ServerKind::OpenAI,
            models,
            selected_service: String::new(),
            endpoint: String::new(),
        })
    }
    pub fn active_service(&self) -> &Value {
        self.service(&self.selected_service)
    }
    pub fn model_summary(&self) -> String {
        if self.models.is_empty() {
            "Sin modelos activos".into()
        } else {
            self.models.join(" · ")
        }
    }
    pub fn n(&self, path: &str) -> f64 {
        number(self.data.pointer(path).unwrap_or(&Value::Null))
    }
    pub fn s(&self, path: &str) -> String {
        string(self.data.pointer(path).unwrap_or(&Value::Null))
    }
    pub fn service(&self, id: &str) -> &Value {
        self.data["services"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == id))
            .unwrap_or(&Value::Null)
    }
    pub fn consumers(&self) -> String {
        let labels: Vec<String> = self.data["vllmThroughput"]["consumers"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|v| v.is_object())
                    .map(|c| {
                        let label = c["label"]
                            .as_str()
                            .or(c["id"].as_str())
                            .unwrap_or("consumidor");
                        let count = number(&c["requests"]);
                        if count > 1. {
                            format!("{label} x{count:.0}")
                        } else {
                            label.into()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        if labels.is_empty() {
            "sin peticiones activas".into()
        } else {
            labels.join("  ")
        }
    }
    pub fn bar_alerts(&self, c: &Config) -> [AlertLevel; 3] {
        [
            AlertLevel::for_value(self.n("/gpu/0/utilization"), c.gpu_warn, c.gpu_danger).max(
                AlertLevel::for_value(self.n("/gpu/0/temperature"), c.temp_warn, c.temp_danger),
            ),
            AlertLevel::for_value(self.n("/memory/percent"), c.mem_warn, c.mem_danger),
            // A high generation rate is throughput, not resource pressure.
            AlertLevel::Normal,
        ]
    }
    pub fn alert_summary(&self, c: &Config) -> String {
        let mut causes = Vec::new();
        for (name, value, unit, warn, critical) in [
            (
                "Uso de GPU",
                self.n("/gpu/0/utilization"),
                "%",
                c.gpu_warn,
                c.gpu_danger,
            ),
            (
                "Temperatura de GPU",
                self.n("/gpu/0/temperature"),
                " °C",
                c.temp_warn,
                c.temp_danger,
            ),
            (
                "RAM",
                self.n("/memory/percent"),
                "%",
                c.mem_warn,
                c.mem_danger,
            ),
        ] {
            let level = AlertLevel::for_value(value, warn, critical);
            if level != AlertLevel::Normal {
                causes.push(format!(
                    "{name}: {}{unit} ({})",
                    decimal(value),
                    if level == AlertLevel::Critical {
                        "umbral crítico"
                    } else {
                        "aviso"
                    }
                ));
            }
        }
        causes.join(" · ")
    }
    pub fn bar(&self) -> [String; 3] {
        [
            if self.n("/gpu/0/utilization") > 100. {
                ">100".into()
            } else {
                percent(self.n("/gpu/0/utilization"))
            },
            gib_short(self.n("/memory/used")),
            compact(self.tps),
        ]
    }
}
fn discover_kairos_models(data: &Value) -> Vec<String> {
    let mut models = Vec::new();
    if let Some(services) = data["services"].as_array() {
        for service in services {
            if service["running"] == false {
                continue;
            }
            if let Some(model) = service["model"].as_str().filter(|m| !m.trim().is_empty()) {
                // Router aliases pointing to another service are not additional model names.
                if !services.iter().any(|s| s["id"] == model) && !models.iter().any(|m| m == model)
                {
                    models.push(model.to_owned());
                }
            }
        }
    }
    let throughput = &data["vllmThroughput"];
    if throughput["ready"] != false
        && throughput["ok"] != false
        && let Some(model) = throughput["model"]
            .as_str()
            .filter(|m| !m.trim().is_empty())
    {
        let stopped = data["services"].as_array().is_some_and(|services| {
            services
                .iter()
                .any(|s| s["model"] == model && s["running"] == false)
                && !services
                    .iter()
                    .any(|s| s["model"] == model && s["running"] == true)
        });
        if !stopped && !models.iter().any(|m| m == model) {
            models.push(model.to_owned());
        }
    }
    models
}
pub fn decimal(n: f64) -> String {
    if n < 0. || !n.is_finite() {
        "--".into()
    } else if n >= 10. {
        format!("{:.0}", n.round())
    } else {
        format!("{:.1}", (n * 10.).round() / 10.)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
/// Keep high throughput values readable without changing menu-bar width.
pub fn compact(n: f64) -> String {
    if n < 0. || !n.is_finite() {
        return "—".into();
    }
    if n >= 999.5e18 {
        return ">99E".into();
    }
    for (scale, suffix) in [
        (1e18, "E"),
        (1e15, "P"),
        (1e12, "T"),
        (1e9, "B"),
        (1e6, "M"),
        (1e3, "k"),
    ] {
        if n >= scale * 0.9995 {
            return format!("{}{suffix}", decimal(n / scale));
        }
    }
    decimal(n)
}
pub fn percent(n: f64) -> String {
    if n < 0. {
        "--".into()
    } else {
        format!("{}%", decimal(n.round()))
    }
}
pub fn gib(n: f64) -> String {
    if n < 0. {
        "--".into()
    } else {
        format!("{:.0} GiB", (n / 1073741824.).round())
    }
}
pub fn gib_short(n: f64) -> String {
    if n < 0. || !n.is_finite() {
        return "—".into();
    }
    let mut value = n / 1073741824.;
    for unit in ["G", "T", "P", "E"] {
        if value < 999.5 {
            return format!("{}{unit}", decimal(value));
        }
        value /= 1024.;
    }
    ">99E".into()
}
pub fn rate(n: f64) -> String {
    if n < 0. {
        return "--".into();
    }
    for (unit, scale) in [("G", 1073741824.), ("M", 1048576.), ("K", 1024.)] {
        if n >= scale {
            let v = if unit == "K" {
                (n / scale).round()
            } else {
                (n / scale * 10.).round() / 10.
            };
            return format!("{v}{unit}");
        }
    }
    format!("{n:.0}")
}
pub fn uptime(n: f64) -> String {
    if n < 0. {
        return "--".into();
    }
    let n = n.round() as u64;
    if n >= 86400 {
        format!("{}d {}h", n / 86400, n % 86400 / 3600)
    } else if n >= 3600 {
        format!("{}h {:02}m", n / 3600, n % 3600 / 60)
    } else {
        format!("{}m", n / 60)
    }
}
pub use crate::network::Client;
pub fn fetch(c: &Config) -> Result<Sample, String> {
    objc2::rc::autoreleasepool(|_| Client::new().fetch(c))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_schema() {
        let s = Sample::parse(
            include_bytes!("../tests/fixtures/metrics.json"),
            "vllmcritic",
        )
        .unwrap();
        assert_eq!(s.bar(), ["0%", "88G", "0"]);
        assert_eq!(s.n("/cpu/count"), 20.);
        assert_eq!(s.service("vllmcritic")["maxModelLen"], 262144);
    }
    #[test]
    fn null_is_unknown_not_zero() {
        assert_eq!(number(&Value::Null), -1.);
        assert!(Sample::parse(br#"{"memory":{"percent":null}}"#, "vllmcritic").is_err());
    }
    #[test]
    fn stopped_service_hides_stale_tps() {
        let s=Sample::parse(br#"{"memory":{"percent":20},"vllmThroughput":{"tps":80},"services":[{"id":"vllmcritic","running":false}]}"#,"vllmcritic").unwrap();
        assert_eq!(s.tps, -1.);
    }
    #[test]
    fn invalid_responses() {
        for bytes in [b"".as_slice(), b"<html>error</html>", b"[]", b"null", b"{}"] {
            assert!(Sample::parse(bytes, "vllmcritic").is_err());
        }
    }
    #[test]
    fn throughput_unready() {
        let s = Sample::parse(
            br#"{"memory":{"percent":20},"vllmThroughput":{"tps":80,"ready":false}}"#,
            "x",
        )
        .unwrap();
        assert_eq!(s.tps, -1.);
    }
    #[test]
    fn detects_models_and_service_without_manual_ids() {
        let s = Sample::parse(include_bytes!("../tests/fixtures/metrics.json"), "auto").unwrap();
        assert_eq!(s.models, ["Qwen3.8-Flash-Next"]);
        assert_eq!(s.selected_service, "vllmcritic");
        let s = Sample::parse(br#"{"memory":{"percent":20},"services":[{"id":"other","model":"Model A","running":true},{"id":"router","model":"other","running":true},{"id":"stopped","model":"Model B","running":false}],"vllmThroughput":{"model":"Model A","tps":15}}"#, "auto").unwrap();
        assert_eq!(s.selected_service, "other");
        assert_eq!(s.models, ["Model A"]);
        let s = Sample::parse(br#"{"memory":{"percent":20},"services":[{"id":"other","model":"Model A","running":false}],"vllmThroughput":{"model":"Model A","tps":15}}"#, "auto").unwrap();
        assert_eq!(s.tps, -1.);
        assert!(s.models.is_empty());
    }
    #[test]
    fn openai_catalog_deduplicates_without_inventing_telemetry() {
        let s = Sample::from_openai(
            br#"{"object":"list","data":[{"id":"model-a"},{"id":"model-b"},{"id":"model-a"}]}"#,
            "server",
        )
        .unwrap();
        assert_eq!(s.models, ["model-a", "model-b"]);
        assert_eq!(s.kind, ServerKind::OpenAI);
        assert_eq!(s.tps, -1.);
        assert_eq!(s.n("/memory/percent"), -1.);
        assert_eq!(s.bar_alerts(&Config::default()), [AlertLevel::Normal; 3]);
        assert!(
            Sample::from_openai(br#"{"object":"list","data":[]}"#, "server")
                .unwrap()
                .models
                .is_empty()
        );
        assert!(Sample::from_openai(br#"{"data":[{}]}"#, "server").is_err());
        assert!(Sample::from_openai(br#"{"data":[]}"#, "server").is_err());
    }
    #[test]
    fn warning_boundaries_and_unknown_values() {
        for (value, expected) in [
            (84.9, AlertLevel::Normal),
            (85., AlertLevel::Warning),
            (94.9, AlertLevel::Warning),
            (95., AlertLevel::Critical),
            (-1., AlertLevel::Normal),
            (f64::NAN, AlertLevel::Normal),
        ] {
            assert_eq!(AlertLevel::for_value(value, 85., 95.), expected);
        }
    }
    #[test]
    fn alerts_follow_temperature_and_pressure_even_when_displayed_values_are_unchanged() {
        let c = Config::default();
        let mut s =
            Sample::parse(include_bytes!("../tests/fixtures/metrics.json"), &c.service).unwrap();
        let values = s.bar();
        s.data["gpu"][0]["temperature"] = serde_json::json!(c.temp_warn);
        assert_eq!(
            s.bar_alerts(&c),
            [AlertLevel::Warning, AlertLevel::Normal, AlertLevel::Normal]
        );
        s.data["gpu"][0]["temperature"] = serde_json::json!(c.temp_danger);
        s.data["memory"]["percent"] = serde_json::json!(c.mem_danger);
        assert_eq!(s.bar(), values);
        assert_eq!(
            s.bar_alerts(&c),
            [
                AlertLevel::Critical,
                AlertLevel::Critical,
                AlertLevel::Normal
            ]
        );
        s.data["gpu"][0]["temperature"] = serde_json::Value::Null;
        s.data["gpu"][0]["utilization"] = serde_json::json!(c.gpu_warn);
        s.tps = 10000.;
        assert_eq!(s.bar_alerts(&c)[0], AlertLevel::Warning);
        assert_eq!(s.bar_alerts(&c)[2], AlertLevel::Normal);
        s.data["memory"]["percent"] = serde_json::json!(10);
        s.data["gpu"][0]["utilization"] = serde_json::json!(0);
        assert_eq!(s.bar_alerts(&c), [AlertLevel::Normal; 3]);
        assert!(s.alert_summary(&c).is_empty());
    }
    #[test]
    fn formatting() {
        assert_eq!(decimal(0.), "0");
        assert_eq!(decimal(9.95), "10");
        assert_eq!(gib_short(1073741824.), "1G");
        assert_eq!(rate(1536.), "2K");
        assert_eq!(uptime(90061.), "1d 1h");
        assert_eq!(percent(-1.), "--");
        assert_eq!(compact(999.9), "1k");
        assert_eq!(compact(1234.), "1.2k");
        assert_eq!(compact(999_999.), "1M");
        assert_eq!(compact(-1.), "—");
        assert_eq!(compact(999.5e18), ">99E");
        for exponent in 0..=21 {
            for multiplier in [0.9994, 0.9995, 1., 9.95, 999.49, 999.5] {
                assert!(compact(multiplier * 10_f64.powi(exponent)).chars().count() <= 4);
                assert!(
                    gib_short(multiplier * 10_f64.powi(exponent))
                        .chars()
                        .count()
                        <= 4
                );
            }
        }
    }
}
