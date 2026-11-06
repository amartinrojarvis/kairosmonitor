# Cambios

## 0.4.1

- Controles inferiores rediseñados como botones nativos con iconos, fondos visibles, mayor área de clic y separación uniforme.
- Fila propia para las acciones, con margen inferior y un separador respecto al contenido.
- La frecuencia de actualización pasa a la cabecera: ya no se superpone con Actualizaciones.
- Botón Actualizar de ancho estable; la versión disponible aparece en su ayuda accesible.
- Capturas del panel con los controles reales para revisar los modos con gráfico, sin gráfico, sin conexión y OpenAI.

## 0.4.0

- Inicio de sesión registrado con launchd y verificado al guardar; reparación al actualizar y migración del antiguo agente.
- Lectura correcta de los estados `disabled/enabled` y `true/false` de launchctl.
- Lanzador breve mediante LaunchServices: evita instancias duplicadas y permite desactivar el inicio sin cerrar la app.
- Búsqueda diaria opcional de releases estables, búsqueda manual y aviso en el panel.
- Instalación con un clic, verificación SHA-256 y del bundle, reinicio y recuperación ante errores del reemplazo.
- Guía y script de publicación para que las siguientes versiones se detecten automáticamente.

## 0.3.0

- Barra de ancho fijo por configuración, cifras alineadas y márgenes compactos; sin saltos al cambiar los valores o perder la conexión.
- Avisos suaves solo en la cifra de GPU o RAM afectada, con umbrales configurables de uso, temperatura y memoria.
- Detección automática de modelos y del servicio de rendimiento de Kairos.
- Catálogo de modelos de servidores compatibles con OpenAI mediante `/v1/models`.
- Botón Probar conexión, sin guardar cambios, con mensajes de resultado y errores; comparte el trabajador de red existente.
- Nuevo icono de aplicación generado a todas las resoluciones desde código nativo.
- Documentación de instalación, conexión, límites, configuración y verificación.

## 0.2.1

- Barra más compacta y márgenes equilibrados.

## 0.2.0

- Distribución DMG exclusiva para Apple Silicon y ajustes de inicio de sesión.
- Sesión HTTP nativa, peticiones acotadas y temporizador sin sondeo continuo de interfaz.
- Símbolos nativos, fondo transparente y eliminación del marco adicional de la barra.
