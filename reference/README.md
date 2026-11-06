# kairoswidget — widget `amartin.kairos` para la barra de Omarchy

Telemetría de la máquina remota **kairos** (Kairos Dash, `http://kairos:8787/`)
en la barra superior de Omarchy. En la barra es una píldora con tres chips; al
pulsarla se abre un panel con el detalle completo, con estética de terminal
(etiqueta con `$`, valores grandes, medidores finos, gráfico de puntos) pero
pintado **con los colores del tema activo de Omarchy**, no con el verde de Kairos.

## Qué se ve

Píldora de la barra, en este orden (dato externo → primero de la sección derecha):

| # | Icono | Dato | Campo del JSON |
|---|---|---|---|
| 1 | GPU `U+F061A` | uso de GPU (%) | `gpu[0].utilization` |
| 2 | RAM `U+F035B` | RAM del sistema (**GiB usados**) | `memory.used` |
| 3 | rayo `U+F140B` | vllmcritic tokens/s | `vllmThroughput.tps` |

Panel (clic izquierdo): cabecera con hostname, `uptime`, última actualización,
tailscale y punto de estado; tres tarjetas —**uso de GPU**, **system memory** en
GiB y **TPS**—; rejilla con CPU, carga, red, temperatura, disco, modelo, contexto
y consumidores; gráfico `tok/s · últimas 1 min`; pie con ayuda y cadencia.

Interacciones: **clic izquierdo** abre el panel · **clic derecho** abre el
dashboard en el navegador · **clic central** refresca al momento. Si Kairos no
responde, los valores pasan a `--` y la píldora se atenúa.

## Dónde vive

El código **real** está en el directorio de plugins de usuario del shell, que es
donde `omarchy-shell` lo carga:

```
~/.config/omarchy/plugins/amartin.kairos/
├── manifest.json     # id amartin.kairos, kinds ["bar-widget"], entryPoints.barWidget = Panel.qml
├── Panel.qml         # píldora de la barra + panel emergente (fetch, pintado, componentes inline)
└── Model.js          # parseo del /api/metrics, formateo y colores del tema
```

En este proyecto hay **enlaces simbólicos** a esos tres ficheros: editarlos aquí
edita el plugin en vivo.

> **Importante:** al guardar, el log dice `Local plugin changed, reloading` pero
> el widget **ya colocado en la barra no se reinancia** — el panel sigue
> mostrando el código anterior. Para ver los cambios hay que reiniciar el shell:
> `omarchy restart shell`. (Si además sospechas caché QML:
> `rm -rf ~/.cache/quickshell/qmlcache`.)

## Ajustes

Cualquier clave va en la entrada del widget dentro de
`~/.config/omarchy/shell.json` (o con `omarchy bar set`):

```json
{ "id": "amartin.kairos", "interval": 5, "memWarn": 90, "fillAlpha": 0.12 }
```

| Clave | Default | Qué hace |
|---|---|---|
| `url` | `http://kairos:8787/api/metrics` | endpoint del que se leen los datos |
| `dashboard` | `http://kairos:8787/` | URL que abre el clic derecho |
| `service` | `vllmcritic` | nombre del servicio (etiqueta de la tarjeta, estado systemd) |
| `interval` | `5` | segundos entre peticiones con el panel cerrado |
| `openInterval` | `2` | segundos entre peticiones con el panel abierto (alimenta el gráfico) |
| `timeout` | `4` | `--max-time` de curl |
| `memWarn` | `85` | % de RAM a partir del cual el valor se pinta en amarillo |
| `memDanger` | `95` | % de RAM en rojo |
| `gpuWarn` | `95` | % de GPU en amarillo |
| `gpuDanger` | `101` | % de GPU en rojo (`101` = desactivado; en esta máquina va alta siempre) |
| `tempWarn` / `tempDanger` | `78` / `88` | ºC de GPU en amarillo / rojo |
| `fillAlpha` | `0.10` | alfa del fondo de la píldora (0 = transparente) |
| `borderAlpha` | `0.16` | alfa del borde |
| `border` | `true` | `false` para quitar el borde |
| `graph` | `true` | `false` oculta el gráfico de tok/s |
| `history` | `44` | nº de puntos del gráfico |
| `valueChars` | `4` | dígitos reservados por valor en la píldora (ancho fijo) |

### Ancho fijo (nada se mueve)

La fuente de la barra es monoespaciada, así que cada chip reserva un número de
ícones (`glyph` = avance de un glifo, medido con `TextMetrics`) y se los pasa a
`WidgetButton` como `fixedWidth`; los valores se rellenan con espacios al final
(`pad()`). Así la píldora mide siempre lo mismo pase de `9%` a `92%` o de `5` a
`1.2K`, y los widgets de la derecha no bailan.

Lo mismo se aplicó al widget **`system-monitor`** (local), que es un módulo de
barra y no un plugin:

```
~/.config/omarchy/bar/modules/system-monitor.qml
```

Allí las reservas son `cpuChars: 6`, `memChars: 7`, `netChars: 14` (el chip de
red lleva ↓ y ↑ en el mismo texto, y cada tasa se rellena a 4 glifos).

## Colores

No hay colores fijos: `Model.js` lee el `colors.toml` del tema activo
(`Color.currentThemePath`) a través de un `FileView` y mapea
ok→`green`/`color2`, aviso→`yellow`/`color3`, peligro→`red`/`color1`
(más `color9`/`color10`/`color11` como variantes de relleno); en temas claros
sube un poco el brillo con `Qt.lighter`. Cambia de tema y el widget cambia solo.

## Sobrevive a `omarchy update`

Nada de lo que hay aquí pertenece al paquete `omarchy` (verificado con
`pacman -Qo` → *No package owns*, y `pacman -Ql omarchy | grep bar/modules` →
vacío). El paquete instala `/usr/bin/omarchy*` y `/usr/share/omarchy/**`; lo
del usuario vive en `~/.config/omarchy/` y no se toca al actualizar:

| Ruta | ¿La posee un paquete? | Riesgo |
|---|---|---|
| `~/.config/omarchy/plugins/amartin.kairos/` | no | ninguno |
| `~/.config/omarchy/bar/modules/system-monitor.qml` | no | ninguno |
| `~/.config/omarchy/shell.json` (colocación + ajustes) | no | ⚠️ lo reescriben las migraciones y `omarchy reinstall-configs` |

- `omarchy reinstall-configs` hace `cp -af /etc/skel/. ~/`, y
  `/etc/skel/.config/omarchy/` solo trae `branding extensions hooks shell.json
  themed`: no hay `bar/` ni `plugins/`, así que el código está a salvo, pero
  **`shell.json` sí se resetea** → se pierde dónde está puesto el widget y sus
  ajustes. Para recuperarlo: `./restore-bar.sh` (o `cp shell.json.bak
  ~/.config/omarchy/shell.json`).
- `omarchy plugin update` solo itera sobre plugins que son checkout de git
  (`[[ -d $dir/.git ]] || continue`); `amartin.kairos` no tiene `.git`, se salta.
- Lo único que **sí** cambia con las actualizaciones es la API sobre la que se
  pinta: `qs.Ui` (`WidgetButton.fixedWidth`, `KeyboardPanel`, `Style`, `Util`).  
  Si un día tras actualizar el widget se ve raro, mirar ahí primero:
  `pacman -Ql omarchy | grep -E "WidgetButton|KeyboardPanel"`.

## Comandos útiles

```bash
omarchy bar move amartin.kairos --section right --before omarchy.tray   # recolocar
omarchy bar set amartin.kairos interval 10                             # cambiar un ajuste
omarchy plugin disable amartin.kairos                                  # quitarlo de la barra
omarchy plugin enable amartin.kairos --section right                   # volver a ponerlo
omarchy restart shell                                                  # aplicar cambios de código
omarchy-shell amartin.kairos open | close | toggle                     # panel por IPC
journalctl --user --since "-5 min" | grep -iE "kairos|Panel.qml"        # errores QML
```

## Probar el parseo sin tocar la barra

```bash
curl -s http://kairos:8787/api/metrics -o /tmp/m.json && node -e "
const M = require(process.env.HOME + '/.config/omarchy/plugins/amartin.kairos/Model.js');
const s = M.parseMetrics(require('fs').readFileSync('/tmp/m.json','utf8'));
console.log(M.formatPercent(s.gpuPercent), M.formatGiB(s.memUsed), M.formatTps(s.tps));
console.log(M.memTooltip(s)); console.log(M.gpuTooltip(s)); console.log(M.tpsTooltip(s,'vllmcritic'));
"
```

## Notas de implementación (por si vuelve el bicho)

- **Altura 0 en las tarjetas.** Un `Row`/`Column` de QML calcula su
  `implicitHeight` con el `height` de los hijos cuando el hijo lo tiene ligado
  explícitamente. Ligar `card.height → row.height → row.implicitHeight →
  card.implicitHeight` crea un bucle que Qt resuelve con **0**: las tarjetas se
  ven como un hueco vacío. Se rompe con un `readonly property real cardHeight`
  en el `Row` calculado a partir de los `implicitHeight` de las tarjetas.
- **Gráfico sin barras.** Reasignar contra la *misma* array JS
  (`root.tpsHistory = history` tras `history.push(...)`) no emite señal de
  cambio y el `Repeater` no se redibuja. Siempre hay que asignar una array
  nueva (`history.slice()`).
- El panel usa `KeyboardPanel` + el cromo de los paneles del shell (bordes,
  sombras, `Style.*`), igual que los plugins de primer nivel (`omarchy.audio`).
