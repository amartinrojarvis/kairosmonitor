# Kairos Monitor

Monitor nativo de Kairos para la barra de menús de macOS, escrito en Rust y AppKit. **Solo Apple Silicon (M1 o posterior), macOS 12 o posterior.**

## Descargar e instalar

[Descargar KairosMonitor 0.2.0 para Apple Silicon](https://github.com/amartinrojarvis/kairosmonitor/releases/download/v0.2.0/KairosMonitor-0.2.0-arm64.dmg) · [Todas las versiones](https://github.com/amartinrojarvis/kairosmonitor/releases)

1. Abre el DMG y arrastra **KairosMonitor.app** a **Applications / Aplicaciones**.
2. Abre la app. En el primer arranque aparecen los ajustes de conexión.
3. Configura tu endpoint y guarda. Si usas `http://kairos:8787`, el nuevo Mac debe poder acceder a Kairos por Tailscale y resolver ese nombre; también puedes usar su IP.
4. Activa **Abrir al iniciar sesión** si lo deseas. En instalaciones nuevas viene desactivado.

No necesitas Rust, Homebrew ni Terminal. La app incluye todo su código y utiliza las fuentes e iconos de macOS.

La distribución actual tiene firma **ad hoc**, sin Developer ID ni notarización de Apple. Si macOS bloquea la primera apertura, intenta abrir la app y utiliza **Ajustes del Sistema → Privacidad y seguridad → Abrir igualmente**. En macOS 12, está en Preferencias del Sistema → Seguridad y privacidad → General. [Guía oficial de Apple](https://support.apple.com/es-es/102445).

## Barra y panel

- Clic izquierdo: abre o cierra el panel. Escape o clic fuera lo cierran.
- Clic derecho: abre el dashboard. Clic central: actualiza las métricas.
- GPU, RAM y tokens/s se pueden mostrar por separado desde Ajustes; debe quedar al menos una métrica visible.
- La barra es transparente, con símbolos nativos y cifras de ancho uniforme. macOS aplica el color y el resaltado de selección: no hay una píldora adicional que produzca bordes laterales.
- Los avisos de temperatura y RAM se muestran en color en el panel. La barra queda atenuada cuando no hay conexión; los valores grandes se abrevian.

El panel muestra GPU, temperatura, potencia, RAM, swap, rendimiento del servicio, CPU, carga, red, disco, modelo, contexto, consumidores e historial de tokens/s. Un servicio detenido no muestra rendimiento antiguo. Los errores borran la lectura actual y dejan huecos en el historial.

## Consumo y ajustes

- Binario exclusivo `arm64`, optimizado para tamaño, con LTO y símbolos de depuración eliminados.
- Interfaz AppKit: sin navegador, Electron, fuentes empaquetadas ni servidor local.
- Sesión HTTP nativa reutilizable, sin lanzar `curl` en cada consulta y sin caché en disco ni cookies.
- Una petición simultánea, timeout total y respuesta limitada a 2 MiB, incluso con transferencia fragmentada.
- Temporizador de un solo disparo: desaparece el sondeo de interfaz cada 250 ms. El sistema puede agrupar el despertar con otros trabajos.
- La imagen de la barra se vuelve a generar al cambiar sus valores; el panel cerrado no se redibuja.
- Historial acotado: 44 lecturas por defecto, configurable de 8 a 600.

La frecuencia predeterminada es 5 s con el panel cerrado y 2 s abierto; puedes aumentar los intervalos para consultar menos. El consumo real depende del servidor, del intervalo y de si el panel está abierto.

**Ajustes** permite modificar conexión, servicio, intervalos, timeout, historial, umbrales, gráfico, métricas visibles e inicio de sesión. Los cambios se aplican sin reiniciar. Se conservan las configuraciones de la versión 0.1; sus antiguos campos `border`, `fillAlpha` y `borderAlpha` se aceptan por compatibilidad, pero ya no dibujan un marco en la barra.

Configuración: `~/Library/Application Support/KairosMonitor/config.toml`. Editar ese archivo a mano requiere reiniciar. El inicio de sesión es una preferencia de cada Mac y se gestiona con `~/Library/LaunchAgents/es.amartin.kairosmonitor.plist`; activarlo o desactivarlo no abre otra instancia ni cierra la actual. La app debe estar en `/Applications` o `~/Applications` para activarlo. No utiliza `KeepAlive`: **Salir** mantiene la app cerrada hasta abrirla de nuevo o iniciar otra sesión.

Para actualizar, sal de la app y reemplázala en Aplicaciones. Para desinstalar, desactiva el inicio de sesión, guarda, sal y elimina la app. Se conservan tus ajustes.

## Compilar y empaquetar

Requiere herramientas de Xcode, Rust estable y el target `aarch64-apple-darwin`.

```sh
rustup target add aarch64-apple-darwin
./scripts/build-app.sh   # dist/KairosMonitor.app
./scripts/package.sh     # DMG arm64 y SHA256SUMS.txt
./scripts/install.sh     # instala en ~/Applications; conserva ajustes e inicio de sesión
```

La versión del bundle y del DMG se obtiene de `Cargo.toml`. El empaquetado comprueba la firma, el plist, la arquitectura y la integridad del DMG. El icono se genera con AppKit durante la compilación. `CODE_SIGN_IDENTITY` permite seleccionar un certificado de firma disponible; la notarización es un paso de distribución independiente.

## Comprobaciones

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --check
cargo run -- --open
cargo run -- --snapshot /tmp/kairos.png --fixture tests/fixtures/metrics.json
cargo run -- --snapshot /tmp/kairos-offline.png --offline
cargo run -- --snapshot-bar /tmp/kairos-bar.png --fixture tests/fixtures/metrics.json
cargo run -- --snapshot-settings /tmp/kairos-settings.png
```

Las pruebas incluyen reutilización de conexión HTTP, datos frescos sin caché, errores HTTP/JSON, respuestas grandes declaradas y fragmentadas, timeout, migración de ajustes y rutas de inicio de sesión. El control de inicio también puede consultarse desde el binario instalado con `--login-item status` y cambiarse con `--login-item on|off`.

`src/network.rs` gestiona la sesión de red; `src/model.rs` interpreta la telemetría; `src/ui.rs` coordina AppKit y las consultas; `src/drawing.rs` dibuja la barra y el panel; `src/config.rs` valida y guarda ajustes; `src/settings.rs` contiene el formulario; `src/login.rs` gestiona el inicio de sesión.

`reference/` conserva el plugin original de X1 (Omarchy/Quickshell). Kairos Monitor consulta el endpoint directamente: X1 no necesita estar encendido. La fuente histórica y su licencia permanecen en el repositorio, pero ya no se distribuyen dentro de la aplicación.
