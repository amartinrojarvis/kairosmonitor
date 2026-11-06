# Guía para agentes que retomen Kairos Monitor

Lee [docs/DESARROLLO.md](docs/DESARROLLO.md) antes de modificar o publicar la app. Contiene el procedimiento reproducible, las decisiones del proyecto, las regresiones conocidas y la verificación de releases. Consulta también [CHANGELOG.md](CHANGELOG.md), [README.md](README.md) y la guía específica del área que vayas a tocar.

## Criterios del producto acordados con el propietario

- App nativa **solo Apple Silicon**, macOS 12 o posterior. Prioridad: tamaño reducido y mínimo consumo de CPU, memoria y red. Mantener Rust + AppKit, sin Electron, webviews ni servicios residentes adicionales para tareas que ya resuelve el sistema.
- Interfaz y documentación de usuario en español.
- La barra superior tiene **ancho fijo para las métricas seleccionadas**, cifras compactas y márgenes cortos e iguales. Los cambios de valores, avisos y conexión no deben desplazar los demás widgets.
- Los avisos de la barra colorean **solo la cifra afectada**, con tonos suaves. No teñir todo el bloque, sus iconos o separadores, ni añadir un marco que choque con el resaltado nativo.
- Descubrir los modelos automáticamente. Kairos proporciona telemetría; `/v1/models` proporciona un catálogo, no métricas de hardware ni rendimiento. No inventar valores ni requerir identificadores manuales de servicio.
- Conservar los ajustes y la preferencia de inicio de sesión durante instalaciones y actualizaciones. Las pruebas de conexión no guardan cambios.
- Buscar nuevas versiones como máximo una vez al día automáticamente; instalar solo al pulsar **Instalar y reiniciar**. Conservar las verificaciones de descarga y la recuperación del reemplazo.
- Autoría: **Alberto Martín**. Instagram y X: **@amartinro**. Los enlaces están en Ajustes → Acerca de y en el README.

## Forma de trabajar

1. Inspecciona `git status`, la rama, el remoto y la versión real; no deduzcas que el último estado documentado sigue siendo el actual. Conserva cambios del usuario y procesos ajenos al proyecto.
2. Usa los scripts existentes. `build-app.sh` genera el bundle, `install.sh` reemplaza la instalación local, `package.sh` crea el DMG y `release.sh` publica. Lee sus efectos antes de ejecutarlos.
3. La configuración real vive fuera del repositorio. Para pruebas usa `--config` con un archivo temporal y fixtures. Ese argumento **no aísla** el inicio de sesión ni las preferencias del actualizador; consulta la guía antes de probar esas funciones.
4. Verifica lo que cambies: formato y Clippy para código, pruebas relevantes para comportamiento y capturas/UI real para cambios visuales. No añadas tests que solo dupliquen constantes de diseño. El script de publicación ejecuta todas las comprobaciones existentes.
5. Actualiza la documentación afectada. En cambios de aplicación destinados a distribución, sincroniza `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` y el enlace de descarga del README. Los cambios exclusivamente documentales no requieren una nueva release.
6. Si el alcance autorizado incluye publicar, utiliza `scripts/release.sh` desde `main` con el árbol limpio. Publica un número mayor; nunca muevas un tag publicado ni sustituyas el DMG de una release ya distribuida.
7. Antes de dar una publicación por terminada, comprueba remoto, tag, assets, digest y versión instalada cuando corresponda. Informa de qué comprobaste realmente; no afirmes haber probado un reinicio o un segundo Mac si no lo has hecho.

## Ubicación y acceso

Repositorio: <https://github.com/amartinrojarvis/kairosmonitor>. La cuenta y el remoto usan `amartinrojarvis`, sin punto. Git y GitHub CLI son suficientes para el flujo completo; no se necesitan tokens en archivos del proyecto.

En el Mac donde se inició el proyecto, el checkout era `/Users/amartin/proyectos/kairosmonitor`, Cargo estaba en `~/.cargo/bin/cargo`, `gh` en `/opt/homebrew/bin/gh` y la instalación en `~/Applications/KairosMonitor.app`. Son datos de contexto, no rutas que deban imponerse en otros equipos. Los scripts admiten `CARGO_BIN`.
