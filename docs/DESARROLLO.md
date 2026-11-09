# Desarrollo, verificación y publicación de Kairos Monitor

Esta guía permite retomar el proyecto sin depender del historial de conversación. El código y los scripts del checkout son la fuente de verdad; confirma su estado antes de aplicar un procedimiento.

## 1. Punto de partida y documentación

Estado documentado el **6 de noviembre de 2026**: release estable **v0.4.2**, commit de aplicación `a7a3234`, 25 tests superados y Clippy sin advertencias. La entrega de esta guía añade un commit documental posterior; no altera el binario de v0.4.2. Consulta GitHub para saber si ya existen versiones posteriores.

| Documento | Uso |
| --- | --- |
| [AGENTS.md](../AGENTS.md) | Entrada rápida y preferencias que deben preservarse. |
| [README.md](../README.md) | Funciones, instalación, controles, compilación y autoría. |
| [CHANGELOG.md](../CHANGELOG.md) | Cambios distribuidos por versión; origen de las notas de release. |
| [CONEXION.md](CONEXION.md) | URLs, detección de modelos, errores y límites de la API. |
| [ACTUALIZACIONES.md](ACTUALIZACIONES.md) | Contrato del actualizador, inicio de sesión y diagnóstico. |
| [INSTALAR.txt](INSTALAR.txt) | Texto copiado dentro del DMG como `LEEME.txt`. |

Repositorio público: `https://github.com/amartinrojarvis/kairosmonitor.git`. Rama de entrega utilizada: `main`. No hay un workflow de GitHub Actions que publique automáticamente: el procedimiento actual se ejecuta desde un Mac con `scripts/release.sh`.

Al retomar:

```sh
git status --short
git branch --show-current
git remote -v
git log -6 --oneline
gh release view --repo amartinrojarvis/kairosmonitor --json tagName,url
```

Si hay cambios locales, identifica su procedencia antes de editar o preparar commits. No uses un reset para obtener un árbol limpio. No supongas que la versión instalada, la compilada en `dist/` y la última release son iguales.

## 2. Entorno y herramientas

Se desarrolla y verifica en macOS con AppKit. Se necesitan herramientas de Xcode/Command Line Tools, Rust estable compatible con el código del checkout, el target `aarch64-apple-darwin`, Swift, Python 3.9 o posterior y Git. Para publicar se necesita además GitHub CLI autenticada con acceso de escritura al repositorio.

```sh
xcode-select -p
rustc --version
cargo --version
rustup target add aarch64-apple-darwin
swift --version
python3 --version
gh auth status
```

En el entorno original Cargo no siempre estaba en `PATH`: se usó `~/.cargo/bin/cargo`. GitHub CLI estaba en `/opt/homebrew/bin/gh`. Puedes configurar `PATH` para la sesión o proporcionar `CARGO_BIN`; no cambies `HOME` para encontrar las herramientas o probar la app.

Que el repositorio sea público permite leerlo, pero publicar commits, tags o releases requiere autenticación con permisos de escritura. La autenticación de GitHub y la identidad de commits son independientes. Comprueba `git config --get user.name` y `git config --get user.email`. En este trabajo no había identidad Git global; se hicieron commits con opciones por comando, sin modificarla:

```sh
git -c user.name=amartinrojarvis \
    -c user.email=263297183+amartinrojarvis@users.noreply.github.com \
    commit -m "Descripción concreta del cambio"
```

Es la identidad pública usada en el historial, no una credencial. En otro entorno utiliza la identidad autorizada que corresponda. No guardes tokens en scripts, documentación ni commits.

## 3. Arquitectura y responsabilidades

| Archivo | Responsabilidad y puntos de atención |
| --- | --- |
| `src/main.rs` | CLI, configuración y selección de modo. `--launch-at-login` y `--finish-update` son entradas internas; se resuelven antes de abrir la UI. |
| `src/config.rs` | TOML, valores iniciales, validación, edición y guardado. Preserva compatibilidad con campos antiguos. |
| `src/network.rs` | `NSURLSession` efímera reutilizable, delegado serial, streaming acotado, timeout y resolución del endpoint. |
| `src/model.rs` | Telemetría, catálogo OpenAI, elección automática del servicio, abreviaturas y niveles de aviso. |
| `src/ui.rs` | Estado, AppKit, canales del trabajador, temporizador, popover, controles inferiores, capa de cifras coloreadas y Acerca de. |
| `src/drawing.rs` | Geometría, colores, barra, panel de métricas y catálogo. |
| `src/settings.rs` | Formulario, prueba asíncrona, catálogo detectado y preferencias de inicio. |
| `src/login.rs` | Registro, migración y lanzador breve de inicio de sesión. |
| `src/updates.rs` | API de GitHub, comparación de versiones, preferencias diarias, validación del paquete y reemplazo con recuperación. |
| `src/update_window.rs` | Ventana nativa de búsqueda e instalación de versiones. |
| `scripts/icon.swift` | Icono generado con AppKit: una K y un arco de reloj. No se genera durante la ejecución de la app. |
| `tests/fixtures/metrics.json` | Telemetría de referencia para tests y capturas reproducibles. |
| `reference/` | Plugin histórico de X1 y sus recursos/licencias. No es el código de ejecución de la app actual. |

La interfaz corre en el hilo principal. Un único trabajador procesa lecturas, pruebas de conexión, búsquedas de versiones y preparación de actualizaciones. Las respuestas llegan por canales y notificaciones en la cola principal. No pases objetos AppKit entre hilos.

Las lecturas periódicas usan un `NSTimer` de un solo disparo, no un bucle de refresco continuo. La imagen de la barra se conserva mientras sus valores/avisos/configuración no cambian. El panel cerrado no se redibuja. Mantén el historial acotado y las peticiones secuenciales. Una actualización puede retrasar temporalmente las lecturas; esa decisión evita otro trabajador residente.

La telemetría y los metadatos tienen límite de 2 MiB; la descarga del actualizador, 16 MiB. No se ejecuta `curl` en cada lectura. Los procesos auxiliares de empaquetado e instalación se ejecutan solo cuando hacen falta.

## 4. Decisiones y regresiones que conviene recordar

### Barra superior y pie del panel

La barra tiene columnas de capacidad fija, cifras alineadas a la derecha, abreviaturas y margen exterior de 4 puntos. Al cambiar de valor, entrar en aviso o quedar sin conexión no debe cambiar su ancho. Cambiar las métricas visibles o descubrir otro tipo de servidor puede ajustar el ancho una vez.

Desde 0.4.3, `barBackground` elige un fondo suave opcional en la capa del botón nativo, sin cambiar su geometría ni añadir temporizadores. `none` conserva el fondo transparente de las configuraciones anteriores. Es independiente de `background`, que sigue siendo el fondo del panel. Desde 0.4.4, `settings_palette.rs` contiene los botones nativos de la paleta, las muestras en caché y la vista previa clara/oscura con valores de ejemplo. Solo se redibuja al seleccionar un color o exponer la vista. La captura de Ajustes verifica también el target/action de todos los colores, la selección única y su lectura desde el formulario. El selector «Fondo de color» se guarda con el resto del formulario; cancelar o probar conexión no lo aplica.

Los símbolos y cifras normales se dibujan como plantilla nativa. `AlertOverlay` pinta solo las cifras afectadas y devuelve un hit test nulo para no interceptar clics. Se comprobó que temperatura y presión de memoria pueden cambiar el color aunque el texto visible no cambie: la caché debe incluir los niveles de aviso. Mantén los colores suaves y la adaptación clara/oscura. Una tasa alta de tokens/s no es una alarma.

En 0.4.0 el texto «cada 2 s» ocupaba el mismo espacio que Actualizaciones. Desde 0.4.1 la frecuencia está en la cabecera y el pie tiene una fila propia de 50 puntos, botones nativos de 30 puntos, iconos y márgenes uniformes. Usa `footer_buttons` y `footer_button_y` como fuente común para la UI y las capturas; no vuelvas a colocar texto libre encima de los botones. Los paneles con y sin gráfico tienen alturas diferentes.

### Orden de los gráficos (0.5.2)

El historial de tokens/s se dibuja antes del bloque GPT. Este bloque se desplaza 76 pt hacia abajo solo cuando `graph` está activo; sin historial conserva su posición. Se revisaron las capturas de ambos modos y sin conexión, manteniendo la altura y los controles del pie. La publicación 0.5.2 agrupa también los cambios de GPT y ordenación desarrollados localmente como 0.5.0 y 0.5.1.

### Orden de la barra (0.5.1)

`barOrder` guarda una permutación de `gpu`, `memory`, `tps`, `gpt`. Al migrar se conserva el orden histórico y se rechazan duplicados, omisiones o identificadores desconocidos. La visibilidad se guarda por identificador, independientemente del orden. `settings_order.rs` conserva los controles de cada métrica y los recoloca al pulsar las flechas; los extremos se deshabilitan y cada botón tiene etiqueta y ayuda accesibles. El estado es solo del formulario hasta Guardar; Cancelar y Probar conexión no modifican el orden aplicado. La vista previa de la paleta refleja orden y visibilidad, y el orden forma parte de la caché de la barra. La captura de Ajustes ejercita los target/action de las flechas y comprueba que mover una métrica no cambia su selección.

Verificación local de 0.5.1: 28 pruebas superadas, formato y Clippy sin avisos; capturas nativas claras/oscuras, cifras máximas, desconexión y orden inverso mantienen 166 pt con cuatro métricas. Se comprobaron las flechas y la actualización de la vista previa en la app instalada, así como Cancelar y reabrir sin aplicar el cambio. El bundle instalado coincide con `dist/` y conserva el SHA-256 de la configuración anterior. No se publicó una release.

### Uso semanal de GPT (0.5.0)

`src/usage.rs` interpreta la ventana oficial y `weeklyPace` de Kairos Dash. El gráfico nativo conserva relleno real, línea proporcional naranja, diferencia y divisiones reales de los días (no siete segmentos iguales). La fecha de reinicio se presenta en la zona horaria del Mac. La consulta auxiliar se deriva del endpoint `/api/metrics` y tiene una caché de 15 segundos por origen/prefijo, incluidos errores; no añade trabajadores ni temporizadores. Los fallos invalidan GPT sin ocultar hardware. `showGpt` es opcional y vale true al migrar configuraciones. Los umbrales de GPT siguen Dash: 70/90 %.

La barra usa cifras de 10 puntos y símbolos de 9. Desde 0.5.1 no dibuja separadores: los iconos mantienen su posición, las cifras se alinean a la derecha dentro de su capacidad fija y ambos se centran según la altura de las cifras. Mantén las capacidades fijas y la caché sensible al nivel de aviso de GPT, incluso si el porcentaje redondeado no cambia. Desde 0.5.2 el historial de tokens/s aparece encima del gráfico semanal, conservando la altura del panel y el pie. El panel conserva el gráfico semanal en su posición superior si el historial de tokens/s está desactivado. `tests/fixtures/usage.json` contiene una lectura semanal sin metadatos de cuenta; para capturas se puede añadir como `chatgptUsage` a una copia temporal del fixture de métricas.

Verificación local del 9 de noviembre de 2026 para 0.5.0: 27 tests, formato y Clippy sin avisos; bundle arm64 firmado y verificado; capturas con GPT, cifras máximas, sin red, sin historial de tokens y catálogo OpenAI. La barra mide 166 pt con cuatro indicadores frente a 168 pt con tres en 0.4.4; mantiene 166 pt con máximos y sin red. Se abrió el panel real con el 92 % publicado por Kairos y se comprobó la opción GPT en Ajustes. La instalación conserva el SHA-256 de la configuración y su binario coincide con `dist/`. Esta verificación corresponde a una instalación local; no se publicó una release ni se probó otro Mac o un reinicio físico.

### Modelos y conexión

El formulario no requiere `service` ni nombres de modelo. Kairos permite inferir el servicio correspondiente a `vllmThroughput`; se excluyen servicios detenidos y alias de router del catálogo. OpenAI compatible muestra modelos sin inventar GPU/RAM/tokens/s. Se aceptan URLs base, `/v1` y endpoints completos; los prefijos de proxy deben conservarse.

**Probar conexión** usa los campos editados sin guardarlos. Si cambia la URL durante la petición, no se atribuye la respuesta a la nueva URL. Comparte sesión y trabajador con el sondeo. Los HTTP 401/403 tienen un mensaje específico: aún no hay campo de clave API; no sugieras quitar la protección del servidor.

### Inicio de sesión

El código antiguo creaba un plist y habilitaba el servicio, pero no lo registraba en la sesión; el instalador además lo retiraba con `bootout`. También interpretaba solo `=> true`, cuando este macOS devuelve `=> disabled`.

Desde 0.4.0 el agente es `es.amartin.kairosmonitor.login`. Se verifica su registro con `launchctl`, se admiten los dos formatos de estado y se repara al abrir la app instalada si la preferencia seguía activada. El lanzador llama a LaunchServices y termina; no es el proceso de interfaz. Por eso se puede retirar sin cerrar la app. El agente anterior se migra sin matar la instancia que pudiera haber abierto.

No confundas el estado registrado del lanzador con un proceso permanente. `state = not running` con salida 0 es normal. Tras `install.sh`, `open` es asíncrono: una consulta de estado inmediata puede preceder a la reparación de inicio. Espera a que la app haya terminado de abrirse antes de evaluar el resultado.

### Actualizador y autoría

El actualizador acepta la release estable de este repositorio con el nombre exacto `KairosMonitor-VERSION-arm64.dmg`, URL oficial, tamaño y `digest` SHA-256. También verifica la firma íntegra, versión, identificador y macOS mínimo del bundle. No cambies ese contrato unilateralmente: las apps antiguas seguirán usando sus reglas.

El helper de instalación es una copia temporal del ejecutable. Espera a que la UI termine, renombra el bundle anterior como respaldo, coloca el preparado y reabre la app. Hay pruebas de restauración ante fallo de reemplazo o apertura. No garantiza recuperación automática frente a cortes eléctricos o fallos del sistema; no elimines una copia de recuperación sin revisar el estado.

Acerca de usa el panel estándar de AppKit, el icono del bundle y `CARGO_PKG_VERSION`. Los créditos contienen **Alberto Martín** y enlaces HTTPS a `https://www.instagram.com/amartinro/` y `https://x.com/amartinro`. Los rangos de `NSAttributedString` se calculan en UTF-16, no en bytes, para respetar acentos y caracteres especiales.

## 5. Datos del usuario e instalaciones

| Ruta | Contenido |
| --- | --- |
| `~/Library/Application Support/KairosMonitor/config.toml` | Conexión y ajustes reales. |
| `~/Library/Application Support/KairosMonitor/updates.json` | Búsqueda diaria, última comprobación y release disponible. |
| `~/Library/Application Support/KairosMonitor/update-result.txt` | Resultado del último intento de instalación. |
| `~/Library/LaunchAgents/es.amartin.kairosmonitor.login.plist` | Inicio de sesión actual. |
| `~/Library/LaunchAgents/es.amartin.kairosmonitor.plist` | Registro antiguo, si aún no se ha migrado. |
| `~/Library/Caches/KairosMonitor/updates/` | Descargas y helper temporales de actualización. |
| `/Applications/KairosMonitor.app` o `~/Applications/KairosMonitor.app` | Bundle instalado. El script de desarrollo utiliza la segunda ruta. |

No subas estos datos al repositorio. No guardes una URL de pruebas sobre la configuración real: usa un TOML temporal o pulsa Cancelar tras probar. Puedes comparar el SHA-256 del archivo antes y después de una instalación para verificar que no se modificó.

`--config RUTA` aísla la configuración de métricas; **no** cambia las rutas de inicio de sesión ni las preferencias del actualizador. Tampoco aísla el efecto de los scripts de instalación. Para pruebas completamente separadas de esas funciones utiliza otra cuenta o equipo de pruebas.

No abras a la vez la app instalada y un `cargo run` que arranque otra UI durante una prueba de duplicados. `--check` y los modos de captura terminan sin dejar un monitor permanente.

## 6. Ciclo de desarrollo y comprobaciones

1. Revisa el estado inicial y el código afectado. Haz cambios pequeños que respeten los criterios de `AGENTS.md`.
2. Si cambia la aplicación que se distribuirá, elige una versión estable nueva. Modifica `Cargo.toml` y ejecuta `cargo check` para actualizar el lockfile. Las herramientas de empaquetado usan `--locked` y fallan si olvidas este paso.
3. Comprueba formato, comportamiento afectado y UI cuando corresponda. Antes de publicar se ejecutan todas las comprobaciones siguientes mediante el script de release.

```sh
cargo fmt -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

En cambios exclusivamente documentales basta revisar el diff, enlaces locales y ejemplos afectados; no hace falta recompilar, reinstalar, incrementar versión ni ejecutar todos los tests de la app.

Para probar una conexión real, usa la configuración existente solo si es la deseada. El nombre `kairos` depende de la red del usuario, no de un servidor suministrado por el repositorio:

```sh
cargo run -- --check
cargo run -- --config /tmp/kairos-prueba.toml --check
cargo run -- --check-updates
```

Las pruebas unitarias de red usan servidores locales temporales y cubren conexiones reutilizadas, errores HTTP/JSON, timeout y límites de tamaño. No requieren un servidor Kairos real.

### Verificación visual

```sh
cargo run -- --snapshot /tmp/kairos-panel.png --fixture tests/fixtures/metrics.json
cargo run -- --snapshot /tmp/kairos-offline.png --offline
cargo run -- --snapshot-bar /tmp/kairos-bar.png --fixture tests/fixtures/metrics.json
cargo run -- --snapshot-bar /tmp/kairos-native.png --fixture tests/fixtures/metrics.json --native --dark --selected
cargo run -- --snapshot-settings /tmp/kairos-settings.png
```

Inspecciona las imágenes, no solo el código de salida. `--snapshot` incluye los botones reales desde 0.4.1. Para probar sin gráfico, copia `config.example.toml` a un archivo temporal, pon `graph = false` y pásalo con `--config`. Para OpenAI crea un fixture como `{"object":"list","data":[{"id":"Modelo A"}]}`.

Si cambias la barra, compara dimensiones con `sips -g pixelWidth -g pixelHeight` entre lecturas cortas, cifras altas y desconexión. Prepara fixtures derivados con GPU/RAM cerca y por encima de los umbrales; no alteres el fixture base solo para obtener una captura. Revisa colores claros/oscuros, cifras no afectadas, márgenes y clics. El modo `--native` captura el botón real y comprueba que el overlay no intercepte eventos; una captura aislada no sustituye la revisión del menú activo de macOS.

Para controles del panel, revisa los modos con gráfico, sin gráfico, sin red y OpenAI, y abre Ajustes/Actualizaciones desde los botones. Si hay herramientas de automatización de UI disponibles, utiliza el árbol de accesibilidad y capturas para confirmar el resultado. Las capturas temporales no se incluyen automáticamente en el paquete.

### Inicio de sesión y actualización completa

Usa el **binario instalado**, porque registrar desde `target/`, el DMG o App Translocation se rechaza deliberadamente. Guarda el estado inicial, prueba desactivar/activar y confirma que no cierra ni duplica la UI; restaura la preferencia al terminar. Para probar el lanzamiento, sal de la app y ejecuta el `kickstart` específico descrito en [ACTUALIZACIONES.md](ACTUALIZACIONES.md). No reinicies ni cierres la sesión del usuario como parte de una comprobación rutinaria.

Si cambias el actualizador, prueba comparación de versiones, rechazo de digest/asset inválido y recuperación del reemplazo. Una prueba completa necesita una app con número menor y una release oficial mayor verificable. En el desarrollo de 0.4.0 se instaló temporalmente una compilación de prueba numerada 0.2.9, se actualizó desde ella a la release pública 0.3.0 con el botón y se restauró después la compilación final 0.4.0. Se verificaron reapertura, versión instalada, limpieza de temporales y configuración intacta. No se publicó el número de prueba.

Ese procedimiento es un antecedente, no una orden de degradar una instalación actual. Si repites una prueba equivalente, prepara la recuperación antes, utiliza un entorno adecuado y restaura inmediatamente `Cargo.toml`, `Cargo.lock` y la app final. No publiques un tag ficticio ni dejes al usuario con una versión de prueba.

## 7. Construcción, instalación y empaquetado

| Comando | Efecto |
| --- | --- |
| `./scripts/build-app.sh` | Compila release arm64, reconstruye `dist/KairosMonitor.app`, genera icono/plist y firma/verifica. No instala. |
| `./scripts/install.sh` | Compila, retira los agentes del proyecto en la sesión actual, termina la instalación de `~/Applications`, reemplaza ese bundle y lo abre. Preserva archivos de preferencias; la app repara el inicio activado al arrancar. |
| `./scripts/package.sh` | Compila y crea el DMG con app, enlace a `/Applications`, `LEEME.txt` y `dist/SHA256SUMS.txt`. Verifica la integridad del DMG. No instala ni publica. |
| `./scripts/release.sh` | Ejecuta comprobaciones y empaquetado; sube commit/tag y crea la release pública. No instala la app local. |

El binario usa LTO, una unidad de codegen, `opt-level = "s"`, stripping y `panic = "abort"`. Se fija `MACOSX_DEPLOYMENT_TARGET=12.0`. No añadas un target Intel o universal: el propietario pidió exclusivamente Apple Silicon.

La firma por defecto es ad hoc (`-`). `CODE_SIGN_IDENTITY` permite usar un certificado disponible, pero el script no notariza. No describas el DMG como notarizado ni desactives protecciones del sistema para probarlo. Sigue las instrucciones de primera apertura del README cuando proceda.

`target/`, `dist/`, `artifacts/` y `.DS_Store` están ignorados. El icono se regenera si `scripts/icon.swift` es más reciente que `target/KairosMonitor.icns`. Si cambias el icono, revisa sus resoluciones y actualiza también la vista previa `docs/images/icon.png`.

Referencia de tamaño, no garantía: el DMG de 0.4.2 fue aproximadamente 588 KiB y la app instalada aproximadamente 1 MiB según `du`. Compara cambios contra esa escala; no sacrifiques legibilidad o corrección por diferencias mínimas. No se ha hecho un benchmark controlado de CPU/memoria para todas las máquinas.

## 8. Publicación reproducible

Antes de publicar, el alcance de la tarea debe incluir la entrega de esa versión. El script realiza escrituras reales en GitHub; no es un simulacro.

1. Trabaja desde `main`, con el remoto HTTPS oficial y sin cambios ajenos. Revisa si el remoto ha avanzado; integra esos cambios sin forzar el push.
2. Comprueba que la versión nueva es mayor que la última release y que no existe ya su tag.
3. Sincroniza `Cargo.toml` y `Cargo.lock`. Añade la sección `## VERSION` al changelog y actualiza el enlace de descarga del README. Mantén coherentes las guías y `INSTALAR.txt` si cambia el uso.
4. Revisa el diff y haz commit de archivos concretos. Ejecuta `./scripts/release.sh` desde un árbol limpio.

El script comprueba remoto, autenticación, versión, tag y árbol limpio; ejecuta formato/tests/Clippy, empaqueta, extrae notas del changelog, hace `git push origin HEAD:main`, crea/sube `vVERSION` y publica con `gh release create --latest`. Por eso es importante revisar la rama antes: el script no exige por sí mismo que la rama local se llame `main`.

Si `gh` no está en el PATH de ese Mac:

```sh
PATH="/opt/homebrew/bin:$PATH" ./scripts/release.sh
```

El paquete debe llamarse exactamente `KairosMonitor-VERSION-arm64.dmg`. La release debe ser estable, estar publicada y tener ese asset con digest; el cliente no actualiza desde commits sueltos. No necesitas un appcast aparte ni un servidor de actualización propio.

### Comprobación posterior obligatoria para considerar publicada la entrega

En los comandos siguientes `VERSION` se deriva del checkout que acabas de publicar:

```sh
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
git status --short
git rev-parse HEAD
git ls-remote origin refs/heads/main "refs/tags/v$VERSION"
gh api "repos/amartinrojarvis/kairosmonitor/releases/tags/v$VERSION" \
  --jq '{tag:.tag_name,draft:.draft,prerelease:.prerelease,assets:[.assets[]|{name,size,digest}]}'
gh release view --repo amartinrojarvis/kairosmonitor --json tagName,url
(cd dist && shasum -a 256 -c SHA256SUMS.txt)
```

Comprueba que `main` y el tag remoto corresponden al commit esperado, que la nueva release es la última y que el `digest` SHA-256 del DMG coincide con el archivo local, no solo que hay un asset con el mismo nombre. Una publicación completada con éxito no implica que la app de `~/Applications` se haya actualizado.

Si la tarea incluye instalación local, comprueba también:

```sh
plutil -extract CFBundleShortVersionString raw \
  "$HOME/Applications/KairosMonitor.app/Contents/Info.plist"
cmp dist/KairosMonitor.app/Contents/MacOS/kairosmonitor \
  "$HOME/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor"
"$HOME/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor" --login-item status
```

Adapta la ruta si se utiliza `/Applications`. Verifica que la preferencia de inicio y los ajustes se conservan. Para revisar el DMG se puede montar con `hdiutil attach -readonly -nobrowse -mountpoint RUTA_TEMPORAL ...`, comprobar plist/firma/binario y desmontarlo con `hdiutil detach RUTA_TEMPORAL`. No reemplaces el `SHA256SUMS.txt` de una release antigua con el que haya generado otra versión en `dist/`.

## 9. Fallos habituales y recuperación

| Situación | Qué revisar o hacer |
| --- | --- |
| Cargo no aparece | Usa `~/.cargo/bin/cargo` o `CARGO_BIN`; no instales otro toolchain sin comprobar el existente. |
| `--locked` impide compilar tras subir versión | Ejecuta `cargo check`, revisa y añade `Cargo.lock` al commit. |
| Git no tiene identidad | Comprueba la configuración; usa una identidad autorizada por comando o del repositorio, sin alterar globalmente otras tareas. |
| `release.sh` rechaza árbol sucio | Revisa y confirma los cambios correspondientes. No borres cambios del usuario. |
| Fallo después de subir commit/tag | Comprueba qué llegó al remoto y qué assets existen antes de reintentar. El script completo rechaza tags ya existentes. |
| Tag subido pero release aún inexistente | Si el tag apunta al commit validado y conservas su DMG verificado, completa **esa publicación aún no distribuida** con `gh release create` y `--notes-file`. No muevas el tag ni reconstruyas otro contenido atribuyéndolo al mismo commit. |
| Release publicada con contenido incorrecto o ambiguo | No sobrescribas el DMG ni reutilices el número. Corrige y publica una versión mayor. |
| El otro Mac no ofrece la actualización | Comprueba versión instalada, release estable marcada como última, nombre/digest del asset, conectividad y búsqueda manual. La comprobación automática tiene espera de 24 h. |
| La app es anterior a 0.4.0 | Actualización manual inicial: salir, reemplazar con el DMG y abrir. Después puede actualizar desde la app. |
| Inicio parece activado pero no arranca | Revisa el agente nuevo, estado enabled/disabled, ruta del bundle y prueba de lanzamiento. No concluyas que falla solo porque el lanzador aparece como `not running`. |
| Timeout con Kairos | Distingue fallo de red/servidor de formato de datos. Usa `--check`, una prueba local controlada y la URL adecuada. No cambies la configuración real para ocultar el error. |
| Captura correcta pero controles ilegibles en uso real | Revisa el popover activo, sus subviews y capas. Las capturas antiguas anteriores a 0.4.1 no incluían los botones. |
| Aviso de deprecación de `hdiutil` | En el Mac de desarrollo aparecía aunque el DMG se verificaba correctamente. No cambies el empaquetado a comandos nuevos sin revisar compatibilidad con los equipos de desarrollo. |

Para completar una release que todavía no existe, prepara notas exactas en un archivo y utiliza `gh release create "v$VERSION" "dist/KairosMonitor-$VERSION-arm64.dmg" dist/SHA256SUMS.txt --repo amartinrojarvis/kairosmonitor --verify-tag --latest --title "Kairos Monitor $VERSION" --notes-file RUTA_NOTAS`. Primero confirma la situación descrita en la tabla; no es una alternativa para sobreescribir publicaciones existentes.

## 10. Evidencia y límites al entregar

En este desarrollo se verificaron las pruebas automatizadas, Clippy, las capturas y controles nativos, conexión real con Kairos, detección y errores de un servidor OpenAI local de prueba, lanzamiento mediante launchd, ausencia de duplicados al activar/desactivar, instalación completa desde el actualizador, preservación de configuración y publicación con digests coincidentes.

No se verificó un reinicio físico completo, todas las versiones de macOS desde 12 ni la ejecución en un segundo Mac controlado por el usuario. macOS 12 es el objetivo de compatibilidad declarado, no una afirmación de prueba física en esa versión. El actualizador tampoco soporta actualmente claves API para los servidores de modelos, Developer ID/notarización automática, prereleases o paquetes de otros repositorios.

Al terminar una tarea, indica el resultado, versión/commit, comprobaciones realizadas y forma de obtenerlo. Mantén esta guía actualizada si cambia el flujo. Para documentación solamente, publica el commit y deja intacta la última release de aplicación.
