use crate::{config::Config, model::*, ui::State};
use objc2::AnyThread;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::*;
use objc2_foundation::*;

pub const WIDTH: f64 = 660.;
pub fn height(c: &Config) -> f64 {
    if c.graph { 462. } else { 386. }
}
pub fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
pub fn color(hex: &str, alpha: f64) -> Retained<NSColor> {
    let n = u32::from_str_radix(&hex[1..], 16).unwrap_or(0);
    NSColor::colorWithSRGBRed_green_blue_alpha(
        ((n >> 16) & 255) as f64 / 255.,
        ((n >> 8) & 255) as f64 / 255.,
        (n & 255) as f64 / 255.,
        alpha,
    )
}
pub fn fill(r: NSRect, c: &NSColor, radius: f64) {
    c.setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(r, radius, radius).fill();
}
pub fn outline(r: NSRect, c: &NSColor, radius: f64) {
    c.setStroke();
    let p = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(r, radius, radius);
    p.setLineWidth(1.);
    p.stroke();
}
pub fn text(s: &str, r: NSRect, size: f64, c: &NSColor) {
    // AppKit drawing is confined to the main thread; the attribute values match their keys.
    unsafe {
        let font = NSFont::monospacedDigitSystemFontOfSize_weight(
            size,
            if size >= 17. {
                NSFontWeightMedium
            } else {
                NSFontWeightRegular
            },
        );
        let paragraph = NSMutableParagraphStyle::new();
        paragraph.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let attrs = NSDictionary::from_slices(
            &[
                NSFontAttributeName,
                NSForegroundColorAttributeName,
                NSParagraphStyleAttributeName,
            ],
            &[
                &*font as &AnyObject,
                c as &AnyObject,
                &*paragraph as &AnyObject,
            ],
        );
        NSString::from_str(s).drawInRect_withAttributes(r, Some(&attrs));
    }
}
fn label(s: &str, x: f64, y: f64, w: f64, c: &Config) {
    text(
        &s.to_uppercase(),
        rect(x, y, w, 16.),
        10.,
        &color(&c.muted, 1.),
    );
}
pub fn tone(value: f64, warn: f64, danger: f64, c: &Config) -> Retained<NSColor> {
    color(
        if value < 0. {
            &c.muted
        } else if danger > 0. && value >= danger {
            &c.red
        } else if warn > 0. && value >= warn {
            &c.yellow
        } else {
            &c.green
        },
        1.,
    )
}
#[allow(clippy::too_many_arguments)]
fn card(
    x: f64,
    label_text: &str,
    value: &str,
    fraction: f64,
    detail: &str,
    detail2: &str,
    tone: &NSColor,
    c: &Config,
) {
    let w = 200.;
    fill(rect(x, 98., w, 142.), &color(&c.foreground, 0.045), 10.);
    outline(
        rect(x + 0.5, 98.5, w - 1., 141.),
        &color(&c.foreground, 0.09),
        10.,
    );
    label(label_text, x + 12., 111., w - 24., c);
    text(value, rect(x + 12., 131., w - 24., 34.), 25., tone);
    fill(
        rect(x + 12., 175., w - 24., 6.),
        &color(&c.foreground, 0.07),
        2.,
    );
    fill(
        rect(x + 12., 175., (w - 24.) * fraction.clamp(0., 1.), 6.),
        tone,
        2.,
    );
    text(
        detail,
        rect(x + 12., 192., w - 24., 16.),
        10.,
        &color(&c.muted, 1.),
    );
    text(
        detail2,
        rect(x + 12., 211., w - 24., 16.),
        10.,
        &color(&c.muted, 0.85),
    );
}
fn stat(label: &str, value: &str, x: f64, y: f64, w: f64, c: &Config) {
    text(label, rect(x, y, w, 14.), 10., &color(&c.muted, 0.9));
    text(
        value,
        rect(x, y + 18., w, 18.),
        11.,
        &color(&c.foreground, 1.),
    );
}
pub fn panel(state: &State) {
    if state.kind == Some(ServerKind::OpenAI) {
        catalog_panel(state);
        return;
    }
    let c = &state.config;
    let sample = state.sample.as_ref();
    let online = sample.is_some();
    let n = |path: &str| sample.map(|s| s.n(path)).unwrap_or(-1.);
    let s = |path: &str| sample.map(|s| s.s(path)).unwrap_or("--".into());
    fill(
        rect(0., 0., WIDTH, height(c)),
        &color(&c.background, 0.94),
        0.,
    );
    label("Kairos Monitor", 20., 17., 420., c);
    text(
        &if online {
            s("/hostname").to_uppercase()
        } else {
            "KAIROS".into()
        },
        rect(20., 38., 510., 24.),
        17.,
        &color(&c.foreground, 1.),
    );
    let meta = if online {
        format!(
            "Activo {}  ·  actualizado {}  ·  Tailscale {}",
            uptime(n("/uptime")),
            state.updated,
            s("/tailscale/ips/0")
        )
    } else {
        state.error.clone()
    };
    text(&meta, rect(20., 66., 620., 17.), 10., &color(&c.muted, 1.));
    let status_color = color(if online { &c.green } else { &c.red }, 1.);
    fill(rect(548., 28., 92., 30.), &color(&c.foreground, 0.04), 3.);
    outline(
        rect(548.5, 28.5, 91., 29.),
        &status_color.colorWithAlphaComponent(0.45),
        3.,
    );
    fill(rect(558., 39., 7., 7.), &status_color, 1.);
    text(
        if online { "En línea" } else { "Sin red" },
        rect(572., 35., 65., 19.),
        11.,
        &status_color,
    );
    let gpu = n("/gpu/0/utilization");
    let mem = n("/memory/percent");
    let tps = sample.map(|s| s.tps).unwrap_or(-1.);
    let gpu_detail = if online {
        format!(
            "{} · {}°C",
            s("/gpu/0/name"),
            decimal(n("/gpu/0/temperature"))
        )
    } else {
        "sin datos".into()
    };
    card(
        20.,
        "uso de gpu",
        &percent(gpu),
        gpu / 100.,
        &gpu_detail,
        &if online && n("/gpu/0/powerDraw") >= 0. {
            format!("{:.1} W", n("/gpu/0/powerDraw"))
        } else {
            String::new()
        },
        &tone(n("/gpu/0/temperature"), c.temp_warn, c.temp_danger, c),
        c,
    );
    card(
        230.,
        "memoria RAM",
        &gib(n("/memory/used")),
        mem / 100.,
        &if online {
            format!("de {} · {}", gib(n("/memory/total")), percent(mem))
        } else {
            "sin datos".into()
        },
        &if online {
            format!("swap {}", percent(n("/memory/swapPercent")))
        } else {
            String::new()
        },
        &tone(mem, c.mem_warn, c.mem_danger, c),
        c,
    );
    let svc = sample.map(Sample::active_service);
    let service_status = match svc.and_then(|s| s["running"].as_bool()) {
        Some(true) => "servicio activo",
        Some(false) => "servicio detenido",
        None => "",
    };
    let activity = if online {
        format!(
            "{} activas · {} en cola",
            decimal(n("/vllmThroughput/running")),
            decimal(n("/vllmThroughput/waiting"))
        )
    } else {
        "sin tráfico".into()
    };
    let detail = format!(
        "pico {} · {}",
        decimal(n("/vllmThroughput/maxTps")),
        service_status
    );
    card(
        440.,
        "tokens por segundo",
        &decimal(tps),
        if n("/vllmThroughput/maxTps") > 0. {
            tps / n("/vllmThroughput/maxTps")
        } else {
            0.
        },
        &activity,
        &detail,
        &tone(tps, 0., 0., c),
        c,
    );
    fill(rect(20., 253., 620., 1.), &color(&c.foreground, 0.1), 0.);
    let model = if s("/vllmThroughput/model") != "--" {
        s("/vllmThroughput/model")
    } else {
        svc.map(|v| string(&v["model"])).unwrap_or("--".into())
    };
    let context = svc
        .map(|v| number(&v["maxModelLen"]))
        .filter(|v| *v > 0.)
        .map(|v| format!("{:.0}K ctx", v / 1000.))
        .unwrap_or("--".into());
    let disk = sample
        .and_then(|s| s.data["disk"].as_array())
        .and_then(|a| a.iter().find(|v| v["mount"] == "/"))
        .map(|v| percent(number(&v["percent"])))
        .unwrap_or("--".into());
    let load = if n("/cpu/load/0") >= 0. {
        format!("{:.2} / {}", n("/cpu/load/0"), decimal(n("/cpu/count")))
    } else {
        "--".into()
    };
    let consumers = sample.map(|s| s.consumers()).unwrap_or("--".into());
    for (i, (key, val)) in [
        ("CPU", percent(n("/cpu/percent"))),
        ("CARGA", load),
        (
            "RED ↓ ↑",
            format!(
                "{}  {}",
                rate(n("/network/rxPerSec")),
                rate(n("/network/txPerSec"))
            ),
        ),
        (
            "TEMP",
            if n("/temperature/max") >= 0. {
                format!("{}°C", decimal(n("/temperature/max")))
            } else {
                "--".into()
            },
        ),
        ("DISCO /", disk),
        ("MODELO", model),
        ("CONTEXTO", context),
        ("CONSUMIDORES", consumers),
    ]
    .iter()
    .enumerate()
    {
        stat(
            key,
            val,
            20. + (i % 4) as f64 * 157.5,
            266. + (i / 4) as f64 * 46.,
            150.,
            c,
        );
        if *key == "TEMP" {
            text(
                val,
                rect(492.5, 284., 150., 18.),
                11.,
                &tone(n("/temperature/max"), c.temp_warn, c.temp_danger, c),
            );
        }
    }
    text(
        &format!("cada {}s", c.open_interval),
        rect(285., height(c) - 21., 150., 16.),
        10.,
        &color(&c.muted, 0.85),
    );
    if c.graph {
        let seconds = state
            .history
            .front()
            .zip(state.history.back())
            .map(|(a, b)| b.0.duration_since(a.0).as_secs())
            .unwrap_or(0);
        label(&format!("tok/s · últimos {seconds}s"), 20., 364., 620., c);
        fill(rect(20., 385., 620., 48.), &color(&c.foreground, 0.03), 3.);
        outline(
            rect(20.5, 385.5, 619., 47.),
            &color(&c.foreground, 0.12),
            3.,
        );
        let peak = state.history.iter().map(|v| v.1).fold(1., f64::max);
        let count = state.history.len().max(1);
        let gap = 2.0_f64.min(151. / count as f64);
        let width = (604. - gap * (count - 1) as f64) / count as f64;
        for (i, (_, value)) in state.history.iter().enumerate() {
            if *value < 0. {
                continue;
            }
            let ratio = (*value / peak).clamp(0., 1.);
            let h = (32. * ratio).max(2.);
            fill(
                rect(28. + i as f64 * (width + gap), 425. - h, width, h),
                &color(if online { &c.green } else { &c.muted }, 0.35 + 0.6 * ratio),
                1.,
            );
        }
    }
}
fn catalog_panel(state: &State) {
    let c = &state.config;
    fill(
        rect(0., 0., WIDTH, height(c)),
        &color(&c.background, 0.94),
        0.,
    );
    label(
        "Kairos Monitor · API compatible con OpenAI",
        20.,
        17.,
        620.,
        c,
    );
    let title = state
        .sample
        .as_ref()
        .map(|s| s.s("/hostname"))
        .unwrap_or_else(|| "Servidor sin conexión".into());
    text(
        &title,
        rect(20., 38., 620., 28.),
        19.,
        &color(&c.foreground, 1.),
    );
    let detail = if state.sample.is_some() {
        format!("Conectado · actualizado {}", state.updated)
    } else {
        state.error.clone()
    };
    text(
        &detail,
        rect(20., 73., 620., 18.),
        11.,
        &color(&c.muted, 1.),
    );
    label("Modelos disponibles", 20., 112., 620., c);
    let models = state
        .sample
        .as_ref()
        .map(|s| s.models.as_slice())
        .unwrap_or(&[]);
    let capacity = ((height(c) - 220.) / 28.) as usize;
    for (i, model) in models.iter().take(capacity).enumerate() {
        let y = 141. + i as f64 * 28.;
        fill(
            rect(20., y - 3., 620., 25.),
            &color(&c.foreground, if i % 2 == 0 { 0.035 } else { 0.015 }),
            5.,
        );
        text(
            model,
            rect(30., y, 600., 22.),
            13.,
            &color(&c.foreground, 1.),
        );
    }
    if models.is_empty() {
        text(
            if state.sample.is_some() {
                "El servidor no anuncia modelos activos."
            } else {
                "Pulsa Refrescar o revisa la URL en Ajustes."
            },
            rect(20., 145., 620., 22.),
            12.,
            &color(&c.muted, 1.),
        );
    } else if models.len() > capacity {
        text(
            &format!(
                "{} modelos más · lista completa en Ajustes",
                models.len() - capacity
            ),
            rect(20., 141. + capacity as f64 * 28., 620., 20.),
            11.,
            &color(&c.muted, 1.),
        );
    }
    text(
        "Esta API no publica GPU, RAM ni tokens/s.",
        rect(20., height(c) - 60., 620., 22.),
        11.,
        &color(&c.muted, 1.),
    );
}
/// Transparent template: AppKit owns the background, selected state and light/dark tint.
/// No custom pill/frame can collide with the status button's highlight.
pub const BAR_PADDING: f64 = 4.;
pub struct AlertValue {
    pub value: String,
    pub frame: NSRect,
    pub level: AlertLevel,
}
pub struct BarArtwork {
    pub image: Retained<NSImage>,
    pub alerts: Vec<AlertValue>,
}
pub fn soft_alert_color(level: AlertLevel, dark: bool) -> Retained<NSColor> {
    color(
        match (level, dark) {
            (AlertLevel::Warning, false) => "#936B37",
            (AlertLevel::Warning, true) => "#D1AE78",
            (AlertLevel::Critical, false) => "#A45256",
            (AlertLevel::Critical, true) => "#D39396",
            (AlertLevel::Normal, _) => unreachable!("only affected values use custom color"),
        },
        1.,
    )
}
pub fn alert_values(values: &[AlertValue], dark: bool, offset: NSPoint) {
    for value in values {
        let mut frame = value.frame;
        frame.origin.x += offset.x;
        frame.origin.y += offset.y;
        text(
            &value.value,
            frame,
            11.,
            &soft_alert_color(value.level, dark),
        );
    }
}
pub fn bar(state: &State) -> BarArtwork {
    let c = &state.config;
    let catalog = state.kind == Some(ServerKind::OpenAI);
    let values = if catalog {
        [
            state
                .sample
                .as_ref()
                .map(|s| format!("{} modelos", compact(s.models.len() as f64)))
                .unwrap_or_else(|| "— modelos".into()),
            String::new(),
            String::new(),
        ]
    } else {
        state
            .sample
            .as_ref()
            .map(Sample::bar)
            .unwrap_or(["—".into(), "—".into(), "—".into()])
    };
    let levels = state
        .sample
        .as_ref()
        .map(|s| s.bar_alerts(c))
        .unwrap_or([AlertLevel::Normal; 3]);
    let enabled = if catalog {
        [true, false, false]
    } else {
        [c.show_gpu, c.show_memory, c.show_tps]
    };
    let names = if catalog {
        ["cube", "", ""]
    } else {
        ["cpu", "memorychip", "bolt.fill"]
    };
    let capacities = if catalog {
        ["9999 modelos", "", ""]
    } else {
        ["100%", "999G", "999M"]
    };
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(11., unsafe { NSFontWeightRegular });
    let attrs =
        NSDictionary::from_slices(&[unsafe { NSFontAttributeName }], &[&*font as &AnyObject]);
    // Fixed, bounded numeric columns. Right alignment keeps both outside margins at 4 pt.
    // Changes in digits, alert severity or connectivity never resize the menu-bar item.
    let mut items = Vec::new();
    for i in 0..3 {
        if !enabled[i] {
            continue;
        }
        let symbol = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(names[i]),
            None,
        );
        let (w, h) = symbol
            .as_ref()
            .map(|symbol| {
                let size = symbol.size();
                let scale = (13. / size.width).min(13. / size.height);
                ((size.width * scale).ceil(), (size.height * scale).ceil())
            })
            .unwrap_or((0., 0.));
        // The font attribute contains a retained NSFont of the required type.
        let text_width =
            unsafe { NSString::from_str(capacities[i]).sizeWithAttributes(Some(&attrs)) }
                .width
                .ceil()
                + 2.;
        let actual_width =
            unsafe { NSString::from_str(&values[i]).sizeWithAttributes(Some(&attrs)) }
                .width
                .ceil()
                + 1.;
        let gap = if symbol.is_some() { 3. } else { 0. };
        items.push((
            symbol,
            w,
            h,
            &values[i],
            text_width,
            actual_width,
            gap,
            levels[i],
        ));
    }
    const SEPARATOR_GAP: f64 = 8.;
    let width = items
        .iter()
        .map(|(_, w, _, _, tw, _, gap, _)| w + gap + tw)
        .sum::<f64>()
        + SEPARATOR_GAP * items.len().saturating_sub(1) as f64;
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(width.max(1.), 18.));
    #[allow(deprecated)]
    image.lockFocusFlipped(true);
    let alpha = if state.sample.is_some() { 1. } else { 0.4 };
    let ink = NSColor::blackColor().colorWithAlphaComponent(alpha);
    let mut alerts = Vec::new();
    let mut x = 0.;
    for (index, (symbol, w, h, value, tw, actual_width, gap, level)) in items.iter().enumerate() {
        if index > 0 {
            fill(
                rect(x + SEPARATOR_GAP / 2. - 0.25, 4., 0.5, 10.),
                &NSColor::blackColor().colorWithAlphaComponent(0.2),
                0.,
            );
            x += SEPARATOR_GAP;
        }
        if let Some(symbol) = symbol {
            unsafe {
                symbol.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    rect(x, (18. - h) / 2., *w, *h),
                    NSRect::ZERO,
                    NSCompositingOperation::SourceOver,
                    alpha,
                    true,
                    None,
                );
            }
        }
        let frame = rect(x + w + gap + tw - actual_width, 1., *actual_width, 16.);
        if *level == AlertLevel::Normal {
            text(value, frame, 11., &ink);
        } else {
            alerts.push(AlertValue {
                value: (*value).clone(),
                frame,
                level: *level,
            });
        }
        x += w + gap + tw;
    }
    #[allow(deprecated)]
    image.unlockFocus();
    image.setTemplate(true);
    BarArtwork { image, alerts }
}
