mod config;
mod drawing;
mod model;
mod settings;
mod ui;
use config::Config;
use objc2_foundation::{NSString, NSURL};

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontManagerRegisterFontsForURL(
        url: *const std::ffi::c_void,
        scope: u32,
        error: *mut *const std::ffi::c_void,
    ) -> bool;
}
fn register_font() {
    let bundled = std::env::current_exe().ok().and_then(|p| {
        p.parent()
            .map(|p| p.join("../Resources/Fonts/JetBrainsMonoNerdFont-Regular.ttf"))
    });
    let path = bundled.filter(|p| p.exists()).unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/JetBrainsMonoNerdFont-Regular.ttf")
    });
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    // NSURL and CFURL are toll-free bridged. Process scope, no ownership transfer.
    unsafe {
        CTFontManagerRegisterFontsForURL(
            &*url as *const NSURL as *const _,
            0,
            std::ptr::null_mut(),
        );
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|s| s == "--help") {
        println!(
            "KairosMonitor\n  --check              Consulta y verifica el endpoint\n  --open               Abre el panel al iniciar\n  --settings           Abre los ajustes al iniciar\n  --config RUTA        Configuración alternativa\n  --snapshot PNG       Renderiza una captura nativa\n  --fixture JSON       Datos locales para --snapshot\n  --offline            Sin conexión para --snapshot"
        );
        return;
    }
    let value = |key: &str| {
        args.iter()
            .position(|s| s == key)
            .and_then(|i| args.get(i + 1))
    };
    let path = value("--config")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(Config::path);
    let config = Config::load(&path).unwrap_or_else(|e| {
        eprintln!("{}: {e}", path.display());
        std::process::exit(1)
    });
    if args.iter().any(|s| s == "--check") {
        match model::fetch(&config) {
            Ok(s) => {
                println!(
                    "{} · GPU {} · RAM {} · {} tok/s",
                    s.s("/hostname"),
                    s.bar()[0],
                    s.bar()[1],
                    s.bar()[2]
                );
                return;
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    register_font();
    if let Some(dest) = value("--snapshot-settings") {
        settings::snapshot(&config, dest);
        return;
    }
    if let Some(path) = value("--snapshot") {
        let sample = if args.iter().any(|s| s == "--offline") {
            Err("Kairos no responde".into())
        } else if let Some(f) = value("--fixture") {
            std::fs::read(f)
                .map_err(|e| e.to_string())
                .and_then(|d| model::Sample::parse(&d, &config.service))
        } else {
            model::fetch(&config)
        };
        ui::snapshot(config, sample, path);
        return;
    }
    ui::run(
        config,
        path,
        args.iter().any(|s| s == "--open"),
        args.iter().any(|s| s == "--settings"),
    );
}
