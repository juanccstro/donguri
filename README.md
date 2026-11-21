<p align="center">
  <img src="docs/media/banner.png" alt="Donguri" width="720">
</p>

<p align="center">
  <b>Una ardilla pixel-art en el borde de tu pantalla que guarda tu contexto para cualquier IA.</b><br>
  Selecciona, guarda, pregunta y pega. Local, ligera y sin cuentas.
</p>

<p align="center">
  <a href="https://github.com/juanccstro/donguri/releases/latest"><img src="https://img.shields.io/badge/Descargar_para_Windows-D97757?style=for-the-badge&logo=windows&logoColor=white" alt="Descargar para Windows"></a>
</p>

<p align="center">
  <a href="README.en.md">English</a> · Windows 10 / 11 · Gratis
</p>

<p align="center">
  <img src="docs/media/demo.gif" alt="Seleccionas código, tocas la bellota y Donguri lo explica" width="632">
</p>

## Qué hace

| | |
|---|---|
| <img src="docs/media/icon-acorn.png" width="18" alt=""> **Bellota rápida** | Selecciona texto en cualquier app y aparece una bellotita: Preguntar, Resumir, Traducir, Explicar o Guardar. |
| <img src="docs/media/icon-nest.png" width="18" alt=""> **Nidos** | Guarda textos, capturas y archivos por temas. Búscalos y pégalos como contexto donde quieras. |
| <img src="docs/media/icon-ask.png" width="18" alt=""> **Pregunta** | Responde con tu clave de Gemini (tiene nivel gratis), Claude u OpenAI. Sin clave, abre Claude o ChatGPT con tu texto listo para pegar. |
| <img src="docs/media/icon-plug.png" width="18" alt=""> **Tus nidos en Claude** | Se conecta a Claude Desktop y Claude Code: escribe «usa mi nido Viaje» y los usa con tu suscripción. |
| <img src="docs/media/icon-clock.png" width="18" alt=""> **Pomodoro** | 25/5, 50/10, 90/20 o a tu medida. Su bellota se llena mientras te concentras y duerme en los descansos. |
| <img src="docs/media/icon-lock.png" width="18" alt=""> **Privada** | Todo se queda en tu PC, las claves van al Administrador de credenciales de Windows, sin telemetría e invisible en grabaciones. |

## Instalar

1. Descarga **`Donguri_x.y.z_x64-setup.exe`** desde [Releases](https://github.com/juanccstro/donguri/releases/latest).
2. Ábrelo. Si Windows avisa *«Windows protegió tu PC»*, pulsa **Más información → Ejecutar de todas formas** (la app aún no está firmada).
3. Sigue la **configuración inicial**: idioma, cómo la quieres, bellota rápida, privacidad, IA y si arranca con Windows.

No pide permisos de administrador y se desinstala desde *Configuración → Aplicaciones*.

## Cómo se usa

| Gesto | Qué pasa |
|---|---|
| **Clic** en la ardilla | Menú: Preguntar, Nidos, Pegar nido, Pomodoro y Ajustes |
| **Clic derecho** | Guardar, capturar, ocultar, modos, tamaño y más |
| **Arrastrarla** | Cambia de borde (en cualquier monitor) |
| **Soltar** texto o archivos encima | Los guarda en un nido |
| **Ctrl+V** con el ratón encima | Le da lo que tengas copiado |
| `Alt+Shift+S` · `Alt+Shift+X` | Guardar la selección · Capturar una zona |
| `Alt+Shift+Space` · `Alt+Shift+B` | Pregunta rápida · Mostrar u ocultar |

Todo se cambia en **Ajustes**, y la configuración inicial se repite desde *Ajustes → Ayuda*.

## Galería

<p align="center">
  <img src="docs/media/menu.png" alt="Menú radial" width="420">
  <img src="docs/media/answer.png" alt="Respuesta en el bocadillo" width="300">
</p>
<p align="center">
  <img src="docs/media/kura.png" alt="Nidos" width="300">
  <img src="docs/media/pomodoro-panel.png" alt="Pomodoro" width="250">
  <img src="docs/media/pomodoro.gif" alt="La bellota se llena durante el Pomodoro" width="160">
</p>
<p align="center">
  <img src="docs/media/setup-squirrel.png" alt="Configuración inicial" width="270">
  <img src="docs/media/setup-quick.png" alt="Bellota rápida" width="270">
  <img src="docs/media/setup-ai.png" alt="IA" width="270">
</p>
<p align="center">
  <img src="docs/media/squirrel.gif" alt="Modo Viva" width="160">
  <img src="docs/media/looks.png" alt="Colores y accesorios" width="440">
</p>

## Compilar desde el código

Requiere [Node.js](https://nodejs.org) 22+, [Rust](https://rustup.rs) y los [requisitos de Tauri](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev     # desarrollo
npm run tauri build   # instalador en src-tauri/target/release/bundle/nsis
```

Hecho con Tauri 2, Rust y Svelte 5.

## Licencia

© 2026 [juanccstro](https://github.com/juanccstro). Código visible, no de código abierto: puedes usar Donguri y proponer cambios con *pull requests*, pero no publicar, vender ni redistribuir copias o versiones propias. Detalles en [LICENSE](LICENSE).
