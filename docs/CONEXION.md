# Conexión y modelos

## Kairos

Introduce la URL base (`http://kairos:8787`) o la URL completa de métricas (`http://kairos:8787/api/metrics`). Pulsa **Probar conexión** y después **Guardar**. El monitor descubre los nombres de modelo en los servicios y en el rendimiento publicado por Kairos. No es necesario configurar `vllmcritic` ni otro identificador manualmente.

Se excluyen servicios detenidos y alias que apuntan a otro servicio, y se eliminan duplicados. El rendimiento mostrado corresponde al servicio asociado por Kairos a `vllmThroughput`, no a una suma inventada de todos los modelos.

## Servidores compatibles con OpenAI

Utiliza la URL base, `/v1` o el endpoint completo `/v1/models`. Por ejemplo, `http://servidor:8000/v1`. Una ruta como `https://servidor/proxy/v1` conserva el prefijo y consulta `https://servidor/proxy/v1/models`.

El catálogo debe contener un array `data` con identificadores `id` de modelo. Un catálogo vacío válido se muestra como una conexión correcta sin modelos activos. Los nombres son los publicados por el servidor; la prueba no garantiza que una inferencia posterior tenga éxito.

La app muestra el catálogo en el panel y su cantidad en la barra. `/v1/models` no proporciona telemetría de hardware ni rendimiento de inferencia, por lo que no se presentan GPU, RAM o tokens/s como ceros. Para obtenerlos usa Kairos. La app no instala ni arranca modelos.

## Qué comprueba el botón

La prueba lee la URL y el tiempo de espera editados, realiza una consulta HTTP y valida la respuesta. No guarda cambios, envía prompts ni modifica el servidor. **Guardar** aplica la conexión al monitor; **Cancelar** conserva la anterior.

La operación se ejecuta fuera del hilo de interfaz y comparte el trabajador y la sesión del monitor. Puede esperar a que termine una lectura en curso. La detección de una URL base puede requerir dos peticiones sucesivas: primero `/api/metrics`, después `/v1/models` si el primero responde 404/405 o su contenido no es compatible. El timeout se aplica a cada petición. Una URL explícita se consulta tal cual.

Una vez detectado el endpoint se reutiliza. Volver a pulsar **Probar conexión** fuerza su detección. Si modificas la URL mientras llega la respuesta, el formulario pide probar la nueva dirección para evitar mostrar un resultado de otra URL.

## Diagnóstico

| Mensaje o síntoma | Comprobación |
| --- | --- |
| Tiempo de espera agotado | Comprueba dirección, puerto, red/Tailscale y disponibilidad. Si el servidor responde lentamente, aumenta el tiempo de espera y repite la prueba. |
| HTTP 401/403 | El endpoint necesita autorización. Esta versión no incorpora configuración de claves API; conserva la protección del servidor. |
| HTTP 404/405 | Comprueba la ruta. Con proxies utiliza `/prefijo/v1` o la ruta completa de métricas/modelos. |
| JSON inválido o sin métricas | La URL podría devolver una página HTML o un formato distinto. Utiliza el endpoint de la API. |
| Respuesta superior a 2 MiB | La respuesta supera el límite de memoria de la app. Utiliza un endpoint cuyo catálogo o telemetría quepa en ese límite. |
| Sin modelos activos | Puede ser un catálogo vacío o servicios detenidos. Comprueba qué publica el servidor. |
| Conecta en un Mac pero no en otro | Comprueba que ambos tienen acceso a la misma red y pueden resolver el nombre del servidor. |

La app conserva los ajustes y la URL localmente; no tiene un servicio externo de sincronización. Las consultas se envían al servidor configurado. El campo **Dashboard** es independiente y se abre con el botón correspondiente o con clic derecho en la barra.
