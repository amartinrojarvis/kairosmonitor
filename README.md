# KairosMonitor

Versión nativa para macOS, escrita en Rust y AppKit, del plugin `amartin.kairos` de X1 (Omarchy/Quickshell). Consulta directamente el endpoint de Kairos por Tailscale; X1 no necesita estar encendido.

En la barra: GPU %, RAM usada en GiB y tokens/s. Ancho fijo, iconos originales JetBrains Mono Nerd Font, colores de aviso y estado atenuado sin conexión. Clic izquierdo abre el panel, derecho abre el dashboard y central refresca. El panel incluye GPU/temperatura/potencia, RAM/swap, TPS/pico/solicitudes, CPU/carga/red/disco/modelo/contexto/consumidores y el gráfico de las últimas 44 lecturas. Escape o clic fuera cierra el panel. Los botones inferiores permiten refrescar, abrir el dashboard, editar ajustes y salir.

El panel reproduce la paleta oscura actual de X1, las etiquetas `$`, tarjetas, medidores y gráfico. La barra utiliza el color de texto de macOS para ser legible sobre su fondo. La paleta queda guardada localmente: no sincroniza cambios posteriores del tema de X1. macOS aporta el borde y la flecha del popover. Los textos largos se abrevian para conservar la cuadrícula; el tooltip de la barra incluye el modelo y los consumidores completos.

## Compilar e instalar

Requiere macOS 12 o posterior, herramientas de línea de comandos de Xcode y Rust estable. La compilación usa la arquitectura del Mac actual.

```sh
./scripts/install.sh
```

Instala `~/Applications/KairosMonitor.app` y un LaunchAgent del usuario para arrancar al iniciar sesión. No pide privilegios de administrador. Salir desde el panel mantiene la aplicación cerrada hasta el siguiente inicio de sesión o arranque manual. Para abrirla manualmente:

```sh
open ~/Applications/KairosMonitor.app
```

**Ajustes** abre un panel nativo con endpoint, dashboard, servicio, intervalos, timeout, tamaño del historial, avisos de RAM/temperatura, gráfico y borde. Valida los datos y permite guardar, cancelar o restablecer los valores del formulario. **Guardar** aplica los cambios sin reiniciar y descarta respuestas pendientes de la conexión anterior. Los valores se guardan en `~/Library/Application Support/KairosMonitor/config.toml`; editar ese archivo a mano requiere reiniciar la app. `config.example.toml` documenta todos los ajustes. El instalador conserva la configuración existente. La cadencia es 5 s cerrado / 2 s abierto; timeout de 4 s. Una única consulta en segundo plano impide solicitudes superpuestas. HTTP fallido, JSON inválido o timeout borran la lectura visible. Datos nulos son desconocidos, y un servicio detenido no muestra TPS antiguos. El gráfico conserva huecos durante fallos y etiqueta su duración real.

Comprobar la conexión sin interfaz:

```sh
./target/release/kairosmonitor --check
```

Desactivar el arranque automático:

```sh
launchctl bootout gui/$(id -u)/es.amartin.kairosmonitor
rm ~/Library/LaunchAgents/es.amartin.kairosmonitor.plist
```

Para desinstalar, además eliminar `~/Applications/KairosMonitor.app`. Los ajustes y logs quedan en `~/Library/Application Support/KairosMonitor/` y `~/Library/Logs/KairosMonitor/`.

## Desarrollo y comprobaciones

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- --open
cargo run -- --snapshot /tmp/kairos.png --fixture tests/fixtures/metrics.json
cargo run -- --snapshot /tmp/kairos-offline.png --offline
```

`src/model.rs` interpreta y formatea la telemetría; `src/ui.rs` gestiona AppKit y el sondeo; `src/drawing.rs` dibuja la barra y el panel; `src/config.rs` valida y guarda ajustes; `src/settings.rs` contiene el formulario nativo. `reference/` conserva los archivos originales leídos de X1 el 6 de noviembre de 2026. La fuente se registra únicamente para este proceso y conserva su licencia en `assets/FONT-LICENSE.txt`.

La interfaz usa los bindings oficiales del proyecto [objc2](https://github.com/madsmtm/objc2) para `NSStatusItem` y `NSPopover`; no usa Electron, navegador embebido ni un servidor local. La compilación local tiene firma ad hoc, sin notarización para distribución a otros Macs.
