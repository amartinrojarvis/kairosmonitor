//! The same official weekly window and proportional budget used by Kairos Dash.
use crate::model::number;
use serde_json::Value;

pub fn weekly(data: &Value) -> Option<&Value> {
    if data["ok"] != true {
        return None;
    }
    let window = data
        .get("weeklyUsage")
        .filter(|v| v.is_object())
        .or_else(|| {
            ["primary", "secondary"]
                .into_iter()
                .map(|key| &data["rateLimits"][key])
                .filter(|v| number(&v["windowDurationMins"]) > 1440.)
                .min_by(|a, b| {
                    (number(&a["windowDurationMins"]) - 10080.)
                        .abs()
                        .total_cmp(&(number(&b["windowDurationMins"]) - 10080.).abs())
                })
        })?;
    (number(&window["usedPercent"]) >= 0.).then_some(window)
}
pub fn used(data: &Value) -> f64 {
    weekly(data)
        .map(|w| number(&w["usedPercent"]).clamp(0., 100.))
        .unwrap_or(-1.)
}
pub fn epoch(iso: &Value) -> Option<f64> {
    use objc2_foundation::{NSISO8601DateFormatOptions, NSISO8601DateFormatter, NSString};
    let value = iso.as_str()?;
    let formatter = NSISO8601DateFormatter::new();
    formatter.setFormatOptions(
        NSISO8601DateFormatOptions::WithInternetDateTime
            | NSISO8601DateFormatOptions::WithFractionalSeconds,
    );
    formatter
        .dateFromString(&NSString::from_str(value))
        .or_else(|| {
            formatter.setFormatOptions(NSISO8601DateFormatOptions::WithInternetDateTime);
            formatter.dateFromString(&NSString::from_str(value))
        })
        .map(|d| d.timeIntervalSince1970())
}
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub struct Pace<'a> {
    pub days: &'a [Value],
    pub real: f64,
    pub limit: f64,
}
pub fn pace(data: &Value, now: f64) -> Option<Pace<'_>> {
    weekly(data)?;
    let p = &data["weeklyPace"];
    let days = p["days"].as_array()?.as_slice();
    let today = days
        .iter()
        .find(|d| d["isToday"] == true)
        .or_else(|| days.last())?;
    let real = number(&today["actualPercent"]);
    if real < 0. {
        return None;
    }
    let limit = epoch(&p["cycleStartIso"])
        .zip(epoch(&p["resetIso"]))
        .filter(|(start, reset)| reset > start)
        .map(|(start, reset)| ((now - start) / (reset - start) * 100.).clamp(0., 100.))
        .unwrap_or_else(|| number(&today["expectedPercent"]));
    (limit >= 0.).then_some(Pace {
        days,
        real: real.clamp(0., 100.),
        limit: limit.clamp(0., 100.),
    })
}
pub fn percentage(value: f64) -> String {
    format!(
        "{}%",
        format!("{value:.1}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .replace('.', ",")
    )
}
pub fn reason(data: &Value) -> &'static str {
    if data["errorCode"] == "codex_auth_expired" {
        "Sesión de Codex caducada en Kairos"
    } else {
        "Sin lectura semanal oficial"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_window_and_exact_pace() {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/usage.json")).unwrap();
        assert_eq!(used(&data), 92.);
        let start = epoch(&data["weeklyPace"]["cycleStartIso"]).unwrap();
        let reset = epoch(&data["weeklyPace"]["resetIso"]).unwrap();
        let p = pace(&data, (start + reset) / 2.).unwrap();
        assert_eq!(p.limit, 50.);
        assert_eq!(p.real, 92.);
        assert_eq!(percentage(32.14), "32,1%");
        assert_eq!(percentage(100.), "100%");
        assert_eq!(pace(&data, start - 10.).unwrap().limit, 0.);
        assert_eq!(pace(&data, reset + 10.).unwrap().limit, 100.);
        let mut invalid = data.clone();
        invalid["ok"] = false.into();
        assert_eq!(used(&invalid), -1.);
        assert!(pace(&invalid, start).is_none());
        invalid = data;
        invalid.as_object_mut().unwrap().remove("weeklyUsage");
        invalid["rateLimits"] = serde_json::json!({"primary":{"windowDurationMins":300,"usedPercent":50},"secondary":{"windowDurationMins":10080,"usedPercent":23}});
        assert_eq!(used(&invalid), 23.);
        assert_eq!(used(&serde_json::json!({"ok":true})), -1.);
    }
}
