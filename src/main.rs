mod config;
mod drawing;
mod login;
mod model;
mod network;
mod settings;
mod ui;
mod update_window;
mod updates;
use config::Config;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|s| s == "--finish-update") {
        let result = args
            .get(index + 1)
            .ok_or_else(|| "Falta el manifiesto".to_owned())
            .and_then(|path| updates::finish_install(std::path::Path::new(path)));
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|s| s == "--check-updates") {
        match updates::check(&model::Client::new()) {
            Ok(Some(release)) => println!("Disponible: {}", release.version),
            Ok(None) => println!("Versión {} al día", updates::CURRENT),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|s| s == "--launch-at-login") {
        if let Err(error) = login::launch() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|s| s == "--help") {
        println!(
            "KairosMonitor\n  --check              Consulta y verifica el endpoint\n  --check-updates      Busca una versión nueva en GitHub\n  --open               Abre el panel al iniciar\n  --settings           Abre los ajustes al iniciar\n  --config RUTA        Configuración alternativa\n  --snapshot PNG       Renderiza una captura nativa\n  --snapshot-bar PNG   Vista de la barra\n  --native --dark --selected  Opciones de captura de la barra\n  --snapshot-settings PNG  Vista de ajustes\n  --login-item status|on|off  Inicio de sesión\n  --fixture JSON       Datos locales para --snapshot\n  --offline            Sin conexión para --snapshot"
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
    let first_run = !path.exists();
    let config = Config::load(&path).unwrap_or_else(|e| {
        eprintln!("{}: {e}", path.display());
        std::process::exit(1)
    });
    if args.iter().any(|s| s == "--check") {
        match model::fetch(&config) {
            Ok(s) => {
                if s.kind == model::ServerKind::OpenAI {
                    println!("{} · OpenAI compatible · {}", s.endpoint, s.model_summary());
                    return;
                }
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
    if let Some(action) = value("--login-item") {
        match action.as_str() {
            "status" => println!("{}", login::enabled()),
            "on" | "off" => {
                if let Err(e) = login::set_enabled(action == "on") {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usa --login-item status|on|off");
                std::process::exit(2);
            }
        }
        return;
    }
    if let Some(dest) = value("--snapshot-settings") {
        settings::snapshot(&config, dest);
        return;
    }
    if let Some(path) = value("--snapshot").or_else(|| value("--snapshot-bar")) {
        let sample = if args.iter().any(|s| s == "--offline") {
            Err("Kairos no responde".into())
        } else if let Some(f) = value("--fixture") {
            std::fs::read(f).map_err(|e| e.to_string()).and_then(|d| {
                model::Sample::parse(&d, "auto")
                    .or_else(|_| model::Sample::from_openai(&d, "Servidor de modelos"))
            })
        } else {
            model::fetch(&config)
        };
        if value("--snapshot-bar").is_some() && args.iter().any(|s| s == "--native") {
            ui::snapshot_native_bar(
                config,
                sample,
                path,
                args.iter().any(|s| s == "--dark"),
                args.iter().any(|s| s == "--selected"),
            );
        } else if value("--snapshot-bar").is_some() {
            ui::snapshot_bar(config, sample, path, args.iter().any(|s| s == "--dark"));
        } else {
            ui::snapshot(config, sample, path);
        }
        return;
    }
    if let Err(error) = login::repair_if_enabled() {
        eprintln!("{error}");
    }
    ui::run(
        config,
        path,
        args.iter().any(|s| s == "--open"),
        first_run || args.iter().any(|s| s == "--settings"),
    );
}
