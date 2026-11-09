use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Separate from the historical panel palette; old configurations remain transparent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarBackground {
    #[default]
    None,
    Blue,
    Sage,
    Lavender,
    Sand,
    Rose,
    Gray,
}
impl BarBackground {
    pub const OPTIONS: [(Self, &'static str); 7] = [
        (Self::None, "Sin color"),
        (Self::Blue, "Azul suave"),
        (Self::Sage, "Salvia"),
        (Self::Lavender, "Lavanda"),
        (Self::Sand, "Arena"),
        (Self::Rose, "Rosa suave"),
        (Self::Gray, "Gris perla"),
    ];
    pub fn hex(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Blue => Some("#86ACCF"),
            Self::Sage => Some("#95B49B"),
            Self::Lavender => Some("#ADA0C9"),
            Self::Sand => Some("#C5B18B"),
            Self::Rose => Some("#C79EA8"),
            Self::Gray => Some("#A7ADB5"),
        }
    }
}

/// Stable identifiers keep stored ordering independent of localized labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarMetric {
    Gpu,
    Memory,
    Tps,
    Gpt,
}
impl BarMetric {
    pub const ALL: [Self; 4] = [Self::Gpu, Self::Memory, Self::Tps, Self::Gpt];
    pub fn index(self) -> usize {
        match self {
            Self::Gpu => 0,
            Self::Memory => 1,
            Self::Tps => 2,
            Self::Gpt => 3,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Gpu => "GPU (%)",
            Self::Memory => "RAM (GiB)",
            Self::Tps => "Tokens/s",
            Self::Gpt => "GPT (%)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub url: String,
    pub dashboard: String,
    pub service: String,
    pub interval: u64,
    pub open_interval: u64,
    pub timeout: u64,
    pub gpu_warn: f64,
    pub gpu_danger: f64,
    pub mem_warn: f64,
    pub mem_danger: f64,
    pub temp_warn: f64,
    pub temp_danger: f64,
    pub fill_alpha: f64,
    pub border_alpha: f64,
    pub border: bool,
    pub graph: bool,
    pub bar_background: BarBackground,
    pub show_gpu: bool,
    pub show_memory: bool,
    pub show_tps: bool,
    pub show_gpt: bool,
    pub bar_order: [BarMetric; 4],
    pub history: usize,
    pub value_chars: usize,
    pub background: String,
    pub foreground: String,
    pub muted: String,
    pub green: String,
    pub yellow: String,
    pub red: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            url: "http://kairos:8787/api/metrics".into(),
            dashboard: "http://kairos:8787/".into(),
            service: "auto".into(),
            interval: 5,
            open_interval: 2,
            timeout: 4,
            gpu_warn: 85.,
            gpu_danger: 95.,
            mem_warn: 85.,
            mem_danger: 95.,
            temp_warn: 78.,
            temp_danger: 88.,
            fill_alpha: 0.04,
            border_alpha: 0.12,
            border: false,
            graph: true,
            bar_background: BarBackground::None,
            show_gpu: true,
            show_memory: true,
            show_tps: true,
            show_gpt: true,
            bar_order: BarMetric::ALL,
            history: 44,
            value_chars: 4,
            background: "#202229".into(),
            foreground: "#F2F3F7".into(),
            muted: "#8B909B".into(),
            green: "#33FF52".into(),
            yellow: "#FFCE65".into(),
            red: "#FF8D87".into(),
        }
    }
}
impl Config {
    pub fn path() -> PathBuf {
        PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Application Support/KairosMonitor/config.toml")
    }
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let c: Self = if path.exists() {
            toml::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            Self::default()
        };
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<(), String> {
        let c = self;
        if !c.show_gpu && !c.show_memory && !c.show_tps && !c.show_gpt {
            return Err("Selecciona al menos una métrica para la barra".into());
        }
        if BarMetric::ALL.iter().any(|m| !c.bar_order.contains(m)) {
            return Err("El orden debe contener GPU, RAM, tokens/s y GPT una sola vez".into());
        }
        for url in [&c.url, &c.dashboard] {
            let valid = url
                .strip_prefix("http://")
                .or_else(|| url.strip_prefix("https://"))
                .is_some_and(|rest| {
                    !rest.is_empty()
                        && !rest.starts_with('/')
                        && !rest.chars().any(char::is_whitespace)
                });
            if !valid {
                return Err("Introduce una URL http:// o https:// con un servidor válido".into());
            }
        }
        if !(1..=3600).contains(&c.interval)
            || !(1..=3600).contains(&c.open_interval)
            || !(1..=60).contains(&c.timeout)
            || !(8..=600).contains(&c.history)
            || !(2..=8).contains(&c.value_chars)
        {
            return Err("Intervalos, timeout, history o valueChars fuera de rango".into());
        }
        for color in [
            &c.background,
            &c.foreground,
            &c.muted,
            &c.green,
            &c.yellow,
            &c.red,
        ] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(format!("Color inválido: {color}"));
            }
        }
        for alpha in [c.fill_alpha, c.border_alpha] {
            if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                return Err("Alfa fuera de rango".into());
            }
        }
        for threshold in [
            c.gpu_warn,
            c.gpu_danger,
            c.mem_warn,
            c.mem_danger,
            c.temp_warn,
            c.temp_danger,
        ] {
            if !threshold.is_finite() || threshold < 0. {
                return Err("Umbral inválido".into());
            }
        }
        if c.service.trim().is_empty() {
            return Err("El servicio no puede estar vacío".into());
        }
        if c.gpu_warn > 100. || c.gpu_danger > 100. || c.gpu_warn >= c.gpu_danger {
            return Err("GPU: el aviso debe ser menor que el crítico (0–100 %)".into());
        }
        if c.mem_warn > 100. || c.mem_danger > 100. || c.mem_warn >= c.mem_danger {
            return Err("RAM: el aviso debe ser menor que el crítico (0–100 %)".into());
        }
        if c.temp_warn >= c.temp_danger || c.temp_danger > 150. {
            return Err("Temperatura: aviso < crítico, máximo 150 °C".into());
        }
        Ok(())
    }
    pub fn edited(
        &self,
        inputs: &[(String, String)],
        graph: bool,
        border: bool,
    ) -> Result<Self, String> {
        let mut value = toml::Value::try_from(self).map_err(|e| e.to_string())?;
        for (key, text) in inputs {
            let previous = value
                .get(key)
                .ok_or_else(|| format!("Ajuste desconocido: {key}"))?;
            let next = match previous {
                toml::Value::Integer(_) => toml::Value::Integer(
                    text.trim()
                        .parse()
                        .map_err(|_| format!("{key}: introduce un número entero"))?,
                ),
                toml::Value::Float(_) => toml::Value::Float(
                    text.trim()
                        .parse()
                        .map_err(|_| format!("{key}: introduce un número"))?,
                ),
                toml::Value::String(_) => toml::Value::String(text.trim().into()),
                _ => return Err(format!("Tipo de ajuste no editable: {key}")),
            };
            value[key] = next;
        }
        value["graph"] = toml::Value::Boolean(graph);
        value["border"] = toml::Value::Boolean(border);
        let config: Self = value
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        self.validate()?;
        let parent = path.parent().ok_or("Ruta de configuración inválida")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        let data = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_preserves_palette_and_applies_fields() {
        let original = Config {
            green: "#112233".into(),
            bar_background: BarBackground::Lavender,
            ..Config::default()
        };
        let edited = original
            .edited(
                &[
                    ("interval".into(), "9".into()),
                    ("url".into(), "http://other:8787/api/metrics".into()),
                ],
                false,
                false,
            )
            .unwrap();
        assert_eq!(edited.interval, 9);
        assert_eq!(edited.green, "#112233");
        assert_eq!(edited.bar_background, BarBackground::Lavender);
        assert!(!edited.graph);
        assert!(!edited.border);
    }
    #[test]
    fn reject_invalid_form() {
        let c = Config::default();
        for (key, val) in [
            ("interval", "0"),
            ("interval", "-1"),
            ("interval", "abc"),
            ("timeout", "61"),
            ("history", "601"),
            ("url", "http://"),
            ("url", "file:///etc/passwd"),
            ("memWarn", "99"),
            ("gpuWarn", "99"),
            ("gpuDanger", "101"),
            ("gpuWarn", "NaN"),
            ("tempDanger", "40"),
            ("memWarn", "NaN"),
            ("service", ""),
        ] {
            assert!(
                c.edited(&[(key.into(), val.into())], true, true).is_err(),
                "{key}={val}"
            );
        }
    }
    #[test]
    fn old_configs_migrate_and_bar_keeps_one_metric() {
        let old: Config = toml::from_str("border = true\nvalueChars = 4").unwrap();
        assert!(old.show_gpu && old.show_memory && old.show_tps && old.show_gpt);
        assert_eq!(old.bar_background, BarBackground::None);
        let mut c = old;
        c.show_gpu = false;
        c.show_memory = false;
        c.show_tps = false;
        c.show_gpt = false;
        assert!(c.validate().is_err());
        c.show_tps = true;
        assert!(c.validate().is_ok());
    }
    #[test]
    fn bar_order_migrates_validates_and_survives_edits() {
        let old: Config = toml::from_str("showGpu = false").unwrap();
        assert_eq!(old.bar_order, BarMetric::ALL);
        let custom: Config =
            toml::from_str(r#"barOrder = ["gpt", "tps", "memory", "gpu"]"#).unwrap();
        custom.validate().unwrap();
        let edited = custom
            .edited(&[("interval".into(), "9".into())], false, false)
            .unwrap();
        assert_eq!(edited.bar_order, custom.bar_order);
        let restored: Config = toml::from_str(&toml::to_string(&edited).unwrap()).unwrap();
        assert_eq!(restored.bar_order, custom.bar_order);
        let mut duplicate = custom;
        duplicate.bar_order[0] = BarMetric::Gpu;
        assert!(duplicate.validate().is_err());
        assert!(toml::from_str::<Config>(r#"barOrder = ["gpu", "memory"]"#).is_err());
        assert!(
            toml::from_str::<Config>(r#"barOrder = ["gpu", "memory", "tps", "other"]"#).is_err()
        );
    }
    #[test]
    fn save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("kairos-config-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let c = Config {
            interval: 7,
            bar_background: BarBackground::Sage,
            ..Config::default()
        };
        c.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.interval, 7);
        assert_eq!(loaded.bar_background, BarBackground::Sage);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
