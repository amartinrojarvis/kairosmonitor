# Cambios

## 0.5.2

- Historial de tokens/s encima del gráfico de uso semanal de ChatGPT en el desplegable, manteniendo la altura del panel y los controles inferiores.
- Indicador independiente del porcentaje semanal oficial de GPT y gráfico de ritmo adaptado de Kairos Dash, con límite proporcional, días y reinicio.
- Barra compacta sin separadores, con iconos y cifras alineados verticalmente.
- Orden de las métricas editable con flechas en Ajustes, con vista previa inmediata y aplicación al guardar.

## 0.5.1

Versión local de desarrollo, incluida en la publicación 0.5.2.

- Barra sin separadores, con iconos y cifras centrados verticalmente y posiciones estables.
- Orden configurable con flechas a izquierda y derecha bajo cada métrica en Ajustes; las casillas se mueven con sus elementos.
- Vista previa que refleja el orden y las métricas seleccionadas antes de guardar. El orden se conserva entre aperturas y actualizaciones.

## 0.5.0

Versión local de desarrollo, incluida en la publicación 0.5.2.

- Indicador independiente de uso semanal de GPT en la barra, configurable desde Ajustes.
- Gráfico de ritmo semanal adaptado de Kairos Dash: uso real, límite proporcional exacto, diferencia, días del ciclo y reinicio.
- Lectura oficial de Kairos cada 15 segundos como máximo; sin estimaciones de consumo ni datos antiguos tras un error.
- Barra más compacta para incorporar GPT sin aumentar el ancho respecto a las tres métricas anteriores.

## 0.4.4

- Ajustes rediseñados en dos columnas, con secciones de conexión, apariencia, actualización, inicio y avisos.
- Paleta de colores visible con selección marcada y vista previa inmediata en apariencia clara y oscura.
- Las vistas previas usan valores de ejemplo; los cambios solo se aplican al guardar.

## 0.4.3

- Ajuste «Fondo de color» para la barra de menús: azul suave, salvia, lavanda, arena, rosa suave y gris perla, o sin color.
- Fondo translúcido y discreto que conserva el ancho fijo, el texto nativo y los avisos de las cifras. Las configuraciones anteriores siguen sin color de fondo.

## 0.4.2

- Ventana nativa Acerca de, accesible desde Ajustes, con autoría de Alberto Martín, icono y versión de la app.
- Enlaces a Instagram y X: @amartinro.

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
