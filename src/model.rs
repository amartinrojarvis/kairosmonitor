use crate::config::Config;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Sample {
    pub data: Value,
    pub tps: f64,
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
impl Sample {
    pub fn parse(raw: &[u8], service: &str) -> Result<Self, String> {
        let data: Value = serde_json::from_slice(raw).map_err(|_| "JSON inválido".to_string())?;
        if !data.is_object() {
            return Err("Respuesta inesperada".into());
        }
        let tp = &data["vllmThroughput"];
        let mut tps = number(&tp["tps"]);
        let svc = data["services"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == service));
        if tp["ready"] == false || tp["ok"] == false || svc.is_some_and(|s| s["running"] == false) {
            tps = -1.;
        }
        if number(&data["memory"]["percent"]) < 0.
            && number(&data["gpu"][0]["utilization"]) < 0.
            && tps < 0.
        {
            return Err("Sin métricas en la respuesta".into());
        }
        Ok(Self { data, tps })
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
    pub fn bar(&self) -> [String; 3] {
        [
            percent(self.n("/gpu/0/utilization")),
            gib_short(self.n("/memory/used")),
            decimal(self.tps),
        ]
    }
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
    if n < 0. {
        "--".into()
    } else {
        format!("{}G", decimal(n / 1073741824.))
    }
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
pub fn fetch(c: &Config) -> Result<Sample, String> {
    let out = std::process::Command::new("/usr/bin/curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--noproxy",
            "*",
            "--max-time",
            &c.timeout.to_string(),
            "--max-filesize",
            "2097152",
            "--",
            &c.url,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "Kairos no responde · {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Sample::parse(&out.stdout, &c.service)
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
    fn formatting() {
        assert_eq!(decimal(0.), "0");
        assert_eq!(decimal(9.95), "10");
        assert_eq!(gib_short(1073741824.), "1G");
        assert_eq!(rate(1536.), "2K");
        assert_eq!(uptime(90061.), "1d 1h");
        assert_eq!(percent(-1.), "--");
    }
}
