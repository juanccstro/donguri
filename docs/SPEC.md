# Bellota / Donguri: especificación v1

> Nombre de trabajo: **Donguri** (app) y **Kura** (ventana de nidos). Ver §2.

## 1. Resumen

**Qué es:** una ardilla pixel-art que vive en el borde de tu pantalla (Windows y macOS). Guarda lo que le das (texto, capturas, PDFs) en "nidos" y te lo devuelve cuando hablas con cualquier IA. También responde ella misma en un bocadillo de cómic, usando tu clave de Gemini, Claude u OpenAI.

**El momento "wow":** seleccionas un párrafo en cualquier sitio, lo sueltas sobre la ardilla y se lo come. Luego, en Claude Desktop, escribes "usa mi nido TFG" y Claude ya lo sabe todo.

**Principios (no se negocian):**

- **Nunca estorba.** Quieta por defecto, nunca roba el foco y se aparta sola cuando molesta.
- **No consume.** Sin animación, ~0% de CPU. Todo funciona por eventos, nada se consulta en bucle.
- **Todo es tuyo.** Datos en local, cero telemetría, claves en el llavero del sistema.
- **Un solo lenguaje visual.** Todo es pixel-art: personaje, bocadillos, iconos, menús y ventanas.

## 2. Nombre: propuestas con trasfondo japonés

Por defecto se queda Bellota. Alternativas:

| Nombre | Significado | Historia |
|---|---|---|
| Donguri (どんぐり) | Bellota | El nombre directo, simple y bonito. Las bellotas son un motivo clásico en la cultura infantil japonesa. |
| Kura (蔵) | Almacén tradicional | El kura es el almacén de muros gruesos donde las familias japonesas protegían del fuego lo más valioso. Tu contexto, a salvo. |
| Mottainai (もったいない) | "Qué desperdicio" | Expresa el pesar por desaprovechar algo valioso. Lema: no desperdicies lo que ya le explicaste a la IA. |
| Takuwae (蓄え) | Reservas guardadas | Lo que se aparta hoy para cuando haga falta, como la ardilla antes del invierno. |

Recomendación: la app se llama **Donguri** y su almacén (la ventana de nidos) se llama **Kura**. Las ardillas olvidan parte de las bellotas que entierran, y de esas bellotas olvidadas nacen robles. Lema posible: *"Lo que guardas hoy, crece mañana."* Antes de decidir, comprobar que el nombre esté libre en GitHub y que tenga dominio disponible.

## 3. Personaje y estilo

**Sprite:** lienzo base de 32×32 px, escalado siempre en múltiplos enteros.

| Tamaño | Escala | Píxeles en pantalla |
|---|---|---|
| Pequeño | ×2 | 64 px |
| Mediano (por defecto) | ×3 | 96 px |
| Grande | ×4 | 128 px |

**Postura según el borde:**

- Borde inferior: sentada.
- Bordes laterales: agarrada en vertical, como en un tronco (mismo sprite girado 90°).
- Borde superior: colgada boca abajo de la cola.

**Paleta base (10 colores máximo):**

| Uso | Color |
|---|---|
| contorno | `#3B2417` |
| pelaje | `#B5651D` |
| pelaje claro | `#E0A060` |
| barriga | `#F4E1C1` |
| bellota | `#C98B3C` |
| caperuza | `#6B4F2A` |
| ojo | `#1A1110` |
| brillo | `#FFFFFF` |
| aviso | `#E8743B` |
| éxito | `#6AA84F` |

**Animaciones (hoja de sprites):**

| Animación | Fotogramas | Cuándo |
|---|---|---|
| idle (respira y mueve la cola) | 4 | Modo viva, de vez en cuando |
| parpadeo | 2 | Cada ~8 s, también en modo quieta |
| dormir | 4 | Tras 10 min sin uso |
| masticar | 6 | Al guardar una bellota |
| pensar (gira una bellota y aparecen puntos) | 6 | Esperando la respuesta de la IA |
| hablar | 4 | Mientras se escribe el bocadillo |
| alerta (orejas arriba y un "!") | 3 | Tiene algo para ti |
| asomarse | 5 | Al salir de su escondite |
| exito (levanta la bellota) | 4 | Guardado o acción completada |
| error (niega con la cabeza) | 4 | Algo falló |
| paseo | 6 | Recorre unos centímetros por el borde (modo viva, poco frecuente) |

**Personalización en Ajustes:**

- Colores: rojiza (por defecto), gris, negra, blanca, y "Ezo" con pinceles en las orejas (la ardilla de Hokkaido).
- Accesorios (capa superpuesta): gorro de lana, gafas, bufanda, auriculares, birrete, hoja de arce.

**Tipografía pixel:** Pixelify Sans para el texto y Silkscreen para títulos (Google Fonts, licencia libre). Comprobar tildes y ñ.

**Bocadillos y ventanas:** marco pixel-art 9-slice, con un pico que siempre apunta a la ardilla.

## 4. Presencia en pantalla

**Colocación:** solo pegada a los bordes. Al soltarla se pega al borde más cercano. Posición guardada por monitor.

**Modos (Ajustes):**

- **Quieta:** solo parpadea.
- **Viva (por defecto):** animaciones útiles de vez en cuando (respirar, cola, algún paseo corto).
- **Escondida:** pestaña de 4 px en el borde. Hover ~300 ms → se asoma. Al alejar el ratón, se esconde a los 2 s.

**Multi-monitor:**

- Nunca sale del área de trabajo de su monitor. Respeta barra de tareas y Dock.
- Si el monitor se desconecta, vuelve al principal; si se reconecta, regresa a su sitio.

**Ocultación automática (cada opción activable por separado):**

- Apps en pantalla completa, juegos y presentaciones.
- Grabación o pantalla compartida: la ventana se **excluye de las capturas** (tú la ves, en grabación/videollamada no aparece).
- Lista negra por app: clic derecho → "Ocultar siempre en [app]".

**Ocultar y recuperar:**

- Clic derecho → Ocultar 1 h / Ocultar hasta mañana / Ocultar en esta app.
- Icono en bandeja / barra de menús → Mostrar / Ocultar.
- Atajo global configurable: `Alt+Shift+B` (`⌥⇧B` en Mac).

**Transparencia:** tras 5 s sin interactuar baja al 60% de opacidad. Vuelve al 100% al pasar el ratón.

**Ratón:** nunca roba el foco. La ventana mide exactamente lo que el sprite.

## 5. Guardar: bellotas y nidos

Una **bellota** es un elemento guardado: texto, captura, PDF (se extrae su texto) o imagen. Un **nido** es un grupo de bellotas sobre un tema. Siempre existe el nido **Sin clasificar**.

| Acción | Atajo por defecto |
|---|---|
| Guardar lo seleccionado en cualquier app | `Alt+Shift+S` |
| Captura por zona (oscurece y arrastras un rectángulo) | `Alt+Shift+X` |
| Arrastrar texto, archivos o imágenes sobre la ardilla | — |
| `Ctrl/Cmd+V` con la ardilla seleccionada | — |

**Flujo al guardar:** mini selector pixel-art junto a la ardilla, "¿Dónde la guardo?":

1. Nido sugerido por la IA (solo con clave configurada). Preseleccionado, se confirma con Enter.
2. El nido activo.
3. Los nidos recientes.
4. ＋ Nuevo nido (nombre en línea).
5. Esc → Sin clasificar.

Después: animación masticar → exito.

**Nido activo:** "en lo que estoy ahora". Se añade solo como contexto a lo que preguntes y aparece como etiqueta en el chat.

**Tamaño:** sin límite. Si un nido supera el presupuesto de contexto del modelo, aviso ⚠ y se usan las bellotas más relevantes (búsqueda de texto).

## 6. Interacción

**Clic izquierdo** → menú circular con 4 iconos: Preguntar, Nidos (Kura), Pegar contexto, Ajustes. El arco se orienta hacia el centro de la pantalla (medio círculo en un borde, un cuarto en una esquina). Nunca se corta.

**Clic derecho** → menú contextual: ocultar, cambiar modo, salir.

**Avisos:** orejas arriba + mini bocadillo con icono:

| Icono | Significado |
|---|---|
| 💬 | Respuesta lista (si cerraste el bocadillo) |
| ✓ | Bellota guardada (1,5 s, sin interrumpir) |
| ⚠ | Nido muy grande, o falta configurar algo |

**Ventana Kura (~440×540 px):** lista de nidos a la izquierda, bellotas a la derecha, buscador arriba. Acciones: renombrar, color, marcar activo, mover bellota, borrar, exportar nido a Markdown.

**Pegar contexto:** eliges un nido y se genera un bloque de contexto limpio que se pega en la app donde estabas.

**Ajustes:** pestañas Ardilla (aspecto, tamaño, modo), Comportamiento (ocultación, transparencia, lista negra), Atajos, IA (proveedores y claves), Integraciones (MCP), Privacidad, Idioma (ES/EN).

## 7. Chat y bocadillo

**Preguntar:** cajita de texto pixel pegada a la ardilla.

- Etiquetas visibles: `📎 Usando tu selección (120 palabras) ✕` y `🌰 Nido: TFG ✕`. Cada una se quita con un clic.
- La ardilla pasa a la animación pensar.

**Respuesta en bocadillo:**

- Texto en directo (streaming) con la animación hablar.
- Formato básico: negritas, listas y código.
- Ancho máx. 360 px; a partir de 280 px de alto, scroll.

**Botones:** Copiar · Pegar en la app · Guardar como bellota · Abrir en Claude/ChatGPT · seguir escribiendo.

**Memoria:** la conversación vive mientras el bocadillo está abierto; se olvida al cerrarlo.

**Acciones rápidas:** con texto seleccionado, `Alt+Shift+Espacio` → Resumir · Traducir · Explicar. Si al abrir Preguntar hay selección, aparecen como sugerencias. Traducir usa por defecto el idioma de la interfaz.

## 8. Conexión con la IA

**Proveedores:** Gemini, Claude y OpenAI, con la clave de API del usuario. Varios configurables, uno por defecto.

El selector de modelo se rellena consultando la lista de modelos de cada proveedor (sin nombres fijos en el código).

**Asistente de claves** (bienvenida y Ajustes → IA), una ficha por proveedor:

- Qué es una clave de API, en dos líneas.
- Botón "Conseguir mi clave" a la página correcta (Gemini: Google AI Studio; Claude: consola de Anthropic; OpenAI: plataforma de OpenAI). **Verificar URLs al implementar.**
- Tres pasos con capturas: inicia sesión → crea la clave → cópiala.
- Campo "Pégala aquí" y botón **Probar clave** (llamada mínima).
- Aviso honesto: Claude Pro / ChatGPT Plus no incluyen la API. Gemini ha ofrecido nivel gratuito: confirmar condiciones y mostrarlo como "la opción gratis".

**Abrir en mi app:** prepara el mensaje con el contexto y lo abre en Claude o ChatGPT. Si la web admite texto pre-rellenado en la URL, se usa (verificar). Si no: portapapeles + abrir app/web + aviso "Pega con Ctrl+V".

**Sin clave:** guardar, nidos y MCP funcionan igual. Solo se desactivan Preguntar, acciones rápidas y sugerencia de nido.

## 9. Integración MCP

Binario `bellota-mcp` instalado con la app: servidor MCP local, **solo lectura**, sobre la base de datos de nidos.

Herramientas:

- `listar_nidos()`
- `buscar(consulta, nido?)`
- `leer_nido(nombre, max_tokens?)`
- `leer_bellota(id)`

Ajustes → Integraciones:

- **Conectar con Claude Desktop:** añade la entrada a su archivo de configuración (con copia de seguridad previa).
- **Conectar con Claude Code:** ejecuta `claude mcp add` en ámbito de usuario.
- Cada uno con botón para deshacer e indicador de conexión activa.

## 10. Privacidad

- Todo en local, sin nube. Datos en la carpeta de datos de la app.
- Claves en el llavero del sistema (Administrador de credenciales / Llavero). Nidos sin cifrar.
- Única conexión de red: la llamada al proveedor de IA elegido, solo cuando preguntas.
- **Modo privado:** un clic desde menú o bandeja. Deja de aceptar contenido y la ardilla se pone un antifaz.
- Telemetría: cero, nunca.

## 11. Arquitectura técnica

**Stack:** Tauri v2 (Rust) + Svelte 5 + TypeScript + Vite. Tauri usa el WebView del sistema (WebView2 / WebKit).

**Ventanas** (transparentes, sin bordes, siempre encima; se crean al usarse y se destruyen al cerrarse):

- `ardilla`: siempre viva, del tamaño exacto del sprite.
- `pestana-borde`: solo en modo escondida.
- `menu`, `bocadillo`, `selector-nido`, `captura`, `kura`, `ajustes`.

**Módulos de Rust:**

| Módulo | Función | Librerías orientativas (verificar versiones) |
|---|---|---|
| ventana | Pegado a bordes, límites del monitor, capas | API de monitores de Tauri |
| ocultacion | Pantalla completa, presentaciones, app en primer plano, exclusión de capturas | Eventos del sistema en Windows (sin bucle); notificaciones de cambio de app en macOS |
| atajos | Atajos globales | plugin global-shortcut de Tauri |
| captura | Captura por zona | xcap |
| seleccion | Simular copiar/pegar, leer y restaurar portapapeles | enigo + plugin de portapapeles |
| almacen | SQLite con búsqueda de texto completo | rusqlite (FTS5) |
| pdf | Extraer texto de PDFs | pdf-extract |
| ia | Clientes Gemini, Claude, OpenAI con streaming | reqwest |
| claves | Llavero del sistema | keyring |
| mcp | Binario `bellota-mcp` | rmcp |

**Rendimiento (objetivos a medir):**

- CPU ~0% en reposo. Animaciones a 8–12 fps; pausadas en modo quieta, oculta o durmiendo.
- Sin consultas en bucle: todo por eventos.
- Memoria < 80 MB en reposo (medir con builds reales).

**Permisos en macOS** (explicados en la bienvenida): Accesibilidad (guardar selección, pegar) y Grabación de pantalla (capturas).

## 12. Distribución sin firma (y gratis)

**Builds:** GitHub Actions con la acción oficial de Tauri, en cada versión:

- Windows: instalador `.exe` y ZIP portable.
- macOS: `.dmg` universal (Apple Silicon e Intel).

**Instalación en una línea:**

- Windows: `irm https://<tu-web>/install.ps1 | iex`
- macOS: `curl -fsSL https://<tu-web>/install.sh | sh` (quita la cuarentena).

**Manual:** Windows → SmartScreen: Más información → Ejecutar de todos modos. macOS → Ajustes → Privacidad y seguridad → Abrir igualmente, o `xattr -dr com.apple.quarantine /Applications/Bellota.app`.

**Actualizaciones:** actualizador de Tauri con su propia clave de firma (gratuita).

**Web:** Astro estático en GitHub Pages. Un scroll: ardilla animada, vídeo de 20 s, instalación, "cómo funciona".

## 13. Plan de construcción

Una fase por sesión: modo plan → implementar → `/clear`.

| Fase | Contenido | Hecho cuando… |
|---|---|---|
| 0. Base | Proyecto Tauri + Svelte, CLAUDE.md, CI vacía | `npm run tauri dev` abre una ventana transparente |
| 1. Presencia | Sprite provisional, bordes, multi-monitor, modos, bandeja, atajo, ocultación automática, transparencia | Se queda en su monitor, se esconde en pantalla completa y no aparece en grabaciones; ~0% CPU en quieta |
| 2. Guardar | SQLite, atajos, captura, arrastrar, Ctrl+V, selector de nido, Kura, exportar | Guardar desde 4 vías y encontrarlo con el buscador |
| 3. IA | Asistente de claves, llavero, 3 proveedores en streaming, bocadillo, acciones rápidas, etiquetas | Preguntar con una selección y ver la respuesta escribirse en el bocadillo |
| 4. Integraciones | bellota-mcp, botones de conexión, Pegar contexto, Abrir en mi app | En Claude Desktop, "usa mi nido X" funciona |
| 5. Pulido | Sprites finales, accesorios, colores, ES/EN, bienvenida, modo privado | Alguien sin conocimientos técnicos la configura sola en 3 min |
| 6. Lanzamiento | Builds en CI, scripts de instalación, README, web, vídeo | Instalarla desde cero en un Windows y un Mac limpios funciona |

## 14. Riesgos y cosas a verificar

- Arrastrar texto desde otras apps se comporta distinto en cada sistema → el atajo de guardar selección es la vía principal.
- Exclusión de capturas en macOS: versiones recientes podrían ignorarla en algunas grabaciones. Probar; si falla, detectar la grabación y ocultar.
- Antivirus: un `.exe` sin firmar que simula teclas puede dar falsos positivos. Mitigación: código abierto, builds reproducibles en CI, enviar a Microsoft si ocurre.
- Coste: las APIs de Claude y OpenAI son de pago. Comunicarlo y destacar Gemini y el MCP (que funciona con la suscripción vía la app de Claude).
- Competencia: los controles flotantes "Pets" de OpenAI. Diferencia: todas las IAs, contexto tuyo y encanto.
- URLs de pre-rellenado de Claude/ChatGPT y enlaces de consolas de claves: verificar en la fase 4.
