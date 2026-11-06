# Versiones, actualización e inicio de sesión

## Publicar una versión nueva

1. Incrementa la versión estable `MAJOR.MINOR.PATCH` en `Cargo.toml` y ejecuta `cargo check` para sincronizar `Cargo.lock`.
2. Actualiza `CHANGELOG.md` y el enlace de descarga del README. Comprueba los cambios y haz commit.
3. Ejecuta `./scripts/release.sh` con Git y GitHub CLI autenticados para `amartinrojarvis/kairosmonitor`.

El script exige un árbol limpio, ejecuta formato, tests y Clippy, genera el paquete arm64 y sus sumas, sube el commit y el tag `vVERSION` y publica la release estable con el DMG y `SHA256SUMS.txt`. Las notas salen de la sección correspondiente del changelog.

También puedes publicar a mano: ejecuta `./scripts/package.sh`, crea un tag `vVERSION` sobre ese commit y una release estable con `dist/KairosMonitor-VERSION-arm64.dmg`. GitHub debe publicar el campo `digest` SHA-256 del asset; la app lo exige. El actualizador consulta la [API de releases de GitHub](https://docs.github.com/en/rest/releases/releases#get-the-latest-release) y compara componentes numéricos: `0.10.0` es posterior a `0.9.0`. Ignora borradores, prereleases y versiones iguales o inferiores. Una release sin paquete compatible no se ofrece para instalar.

No reutilices una versión ni reemplaces su DMG: publica un número mayor. Subir solo código o crear un tag no basta para que los usuarios puedan actualizar.

## Funcionamiento y límites

- La búsqueda automática se realiza como máximo una vez cada 24 horas mientras la app está abierta. La búsqueda manual no necesita esperar ese plazo. Un fallo automático espera al siguiente periodo; se puede reintentar manualmente.
- El aviso aparece en el botón del panel. Descargar e instalar requiere pulsar **Instalar y reiniciar**. No solicita permisos para notificaciones ni crea un servicio de actualización permanente.
- La consulta usa HTTPS contra el repositorio fijo. Solo acepta el DMG arm64 de la versión anunciada en ese repositorio, con tamaño de hasta 16 MiB y SHA-256 coincidente. Comprueba firma íntegra, identificador del bundle, versión y macOS mínimo antes de cerrar la app.
- La firma actual sigue siendo ad hoc. Su comprobación verifica integridad, no una identidad Developer ID: la confianza de la distribución procede del repositorio oficial servido por HTTPS y de su digest.
- El reemplazo se prepara en la misma carpeta que la app para poder renombrarlo sin copiar entre discos durante la instalación. Conserva temporalmente el bundle anterior y lo restaura si falla el reemplazo o la orden de apertura. No puede garantizar recuperación automática ante un corte eléctrico o un fallo del sistema durante ese momento; si queda una carpeta `.KairosMonitor-backup-…app`, conserva la copia y reinstala el DMG manualmente.
- No cambia permisos ni pide elevación. Si la carpeta no permite escribir, utiliza una instalación en `~/Applications` o actualiza manualmente.
- Las preferencias están en `~/Library/Application Support/KairosMonitor/updates.json`. Los temporales viven en `~/Library/Caches/KairosMonitor/updates` y se eliminan al terminar. El resultado del último intento se guarda en `update-result.txt` junto a la configuración.
- Una descarga puede retrasar temporalmente el sondeo de métricas, porque ambas tareas comparten el mismo trabajador de red.

## Inicio de sesión

**Abrir al iniciar sesión** abre la app cuando entras en tu sesión gráfica, también después de encender o reiniciar el Mac. No se ejecuta antes de iniciar sesión.

Desde 0.4.0 se utiliza `~/Library/LaunchAgents/es.amartin.kairosmonitor.login.plist`. Se registra inmediatamente en el dominio gráfico del usuario. El agente ejecuta brevemente `--launch-at-login`, comprueba si la app ya está abierta y la abre mediante LaunchServices cuando falta. No se mantiene un helper en segundo plano ni se usa `KeepAlive`. **Salir** sigue cerrándola hasta el siguiente inicio de sesión o apertura manual.

Se migra el antiguo `es.amartin.kairosmonitor.plist` si estaba activado. El registro anterior se desactiva y se retira su archivo sin terminar una app que dependiera de él. La app repara el registro activado cuando se sustituye o cambia entre `/Applications` y `~/Applications`; no vuelve a activar una preferencia desactivada.

Comprobación desde Terminal, ajustando la ruta si la instalaste en `/Applications`:

```sh
~/Applications/KairosMonitor.app/Contents/MacOS/kairosmonitor --login-item status
launchctl print "gui/$(id -u)/es.amartin.kairosmonitor.login"
```

`state = not running` y `last exit code = 0` son normales para el lanzador: su tarea termina tras abrir la app. El estado de la app es independiente. La prueba completa consiste en cerrar Kairos Monitor y ejecutar `launchctl kickstart "gui/$(id -u)/es.amartin.kairosmonitor.login"`; debe abrirse una sola instancia. No hace falta cerrar la sesión para comprobarlo.

Si macOS bloquea elementos de inicio, revisa Ajustes del Sistema → General → Ítems de inicio. Si falla el registro desde la app, se muestra el error al guardar. Para desinstalar, desactiva la casilla y guarda antes de eliminar la aplicación.
