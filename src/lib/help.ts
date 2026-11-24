// In-app usage guide (Ajustes → Ayuda). {save}, {capture}, {quick}, {toggle},
// {capture_ask} and {clipboard} are replaced with the user's current shortcuts.
import type { IconName } from "./Icon.svelte";

export interface HelpTopic {
  icon: IconName;
  title: string;
  points: string[];
}

const es: HelpTopic[] = [
  {
    icon: "acorn",
    title: "Guardar (dar bellotas)",
    points: [
      "Selecciona un texto en cualquier app y pulsa {save}.",
      "Captura una zona de la pantalla con {capture}: arrastra un rectángulo, Esc cancela.",
      "Arrastra texto, imágenes o PDFs y suéltalos encima de la ardilla.",
      "Con el ratón encima de la ardilla, Ctrl+V le da lo que tengas copiado.",
      "Después elige el nido: Enter guarda en el marcado, Esc lo deja en «Sin clasificar» y un clic fuera lo descarta. El aviso «Guardada» tiene Deshacer y Mover.",
    ],
  },
  {
    icon: "ask",
    title: "Preguntar",
    points: [
      "Clic en la ardilla → Preguntar. Si tenías texto seleccionado, lo usa como contexto.",
      "Con texto seleccionado, {quick} abre Resumir · Traducir · Explicar.",
      "Las etiquetas de arriba (selección, nido) se quitan con su ✕.",
      "Bajo la respuesta: Copiar, Pegar en la app, Guardar como bellota o abrir en Claude/ChatGPT.",
      "Necesita una clave de IA (Ajustes → IA). Lo demás funciona sin ella.",
    ],
  },
  {
    icon: "nest",
    title: "Nidos (Kura)",
    points: [
      "Clic en la ardilla → Nidos. A la izquierda los nidos, a la derecha sus bellotas.",
      "El buscador de arriba busca en todo lo guardado.",
      "Chincheta: marca el nido activo (se añade solo como contexto al preguntar).",
      "Lápiz para renombrar (o doble clic), colores, exportar a Markdown y borrar.",
      "Abre una bellota para verla entera, editarla (lápiz), moverla a otro nido o borrarla.",
      "El botón + de un nido crea una nota escrita a mano.",
    ],
  },
  {
    icon: "paste",
    title: "Pegar nido",
    points: [
      "Clic en la ardilla → Pegar nido → elige un nido.",
      "Lo pega como bloque limpio en la app donde estabas (Claude, ChatGPT, un correo…).",
    ],
  },
  {
    icon: "acorn",
    title: "Bellota rápida",
    points: [
      "Selecciona texto en cualquier app (arrastrando o con doble clic): aparece una bellotita junto al cursor.",
      "Deja el ratón un momento encima: Preguntar · Resumir · Traducir · Explicar · Guardar.",
      "Si sigues trabajando, se va sola. No toca tu portapapeles hasta que eliges algo.",
      "Para quitarla o volver a ponerla: clic derecho en la ardilla → «Bellota rápida», o Ajustes → Comportamiento.",
    ],
  },
  {
    icon: "crop",
    title: "Preguntar sobre una captura",
    points: [
      "{capture_ask}: recorta una zona de la pantalla y pregúntale directamente (Describir · Sacar el texto · Explicar).",
      "Después de una captura normal, «Preguntar sobre ella» hace lo mismo sin guardarla.",
      "En Kura, las imágenes guardadas tienen un botón «Preguntar». Si un nido tiene imágenes, las 3 más recientes van con tus preguntas.",
      "Sin clave de IA: la imagen se copia y se abre Claude o ChatGPT para que la pegues.",
    ],
  },
  {
    icon: "clipboard",
    title: "Historial del portapapeles",
    points: [
      "Está en Kura, debajo de los nidos (o con {clipboard}). Es opcional: actívalo ahí o en Ajustes → Privacidad.",
      "Clic en algo para pegarlo donde estabas; también puedes preguntarle, guardarlo como bellota o borrarlo.",
      "Solo en memoria: se borra al salir. Nunca guarda contraseñas marcadas como privadas ni nada en modo privado.",
    ],
  },
  {
    icon: "clock",
    title: "Pomodoro",
    points: [
      "Clic en la ardilla → Pomodoro. Elige Clásico (25/5), Enfoque (50/10), Profundo (90/20) o tus propios minutos.",
      "Mientras te concentras, su bellota se va llenando de verde; en el descanso se echa a dormir.",
      "Cada cambio de fase te avisa con un mensaje junto a ella, nunca con sonido.",
      "Desde el menú ves la fase y el tiempo; en el panel puedes pausar, reiniciar, saltar fase, cambiar de temporizador o terminar.",
    ],
  },
  {
    icon: "plug",
    title: "Usar tus nidos en Claude",
    points: [
      "Ajustes → Integraciones → Conectar con Claude Desktop (y reinicia Claude).",
      "Luego, en Claude escribe «usa mi nido Viaje». Funciona con tu suscripción, sin clave.",
    ],
  },
  {
    icon: "squirrel",
    title: "Moverla, modos y ocultarla",
    points: [
      "Arrástrala a cualquier borde: se queda pegada y recuerda su sitio en cada pantalla.",
      "Modo Viva: respira y pasea. Quieta: solo parpadea. Escondida: solo asoma la bellota; haz clic en ella para que salga.",
      "Ocultar y volver: {toggle}, o clic en la bellota de la bandeja del sistema.",
      "Se esconde sola en pantalla completa y no sale en grabaciones (Ajustes → Comportamiento).",
      "Clic derecho sobre ella: ocultar 1 hora, hasta mañana o siempre en una app.",
    ],
  },
  {
    icon: "lock",
    title: "Privacidad",
    points: [
      "Todo se guarda en tu equipo; las claves en el llavero de Windows.",
      "Modo privado (clic derecho → Modo privado): no guarda nada y se pone un antifaz.",
      "Solo se conecta a internet para tu IA (cuando preguntas) y, una vez al día, para ver en GitHub si hay versión nueva. Esto último se quita en Ajustes → Privacidad.",
    ],
  },
];

const en: HelpTopic[] = [
  {
    icon: "acorn",
    title: "Saving (feeding acorns)",
    points: [
      "Select text in any app and press {save}.",
      "Capture part of the screen with {capture}: drag a rectangle, Esc cancels.",
      "Drag text, images or PDFs and drop them on the squirrel.",
      "With the mouse over the squirrel, Ctrl+V gives her what you copied.",
      "Then pick a nest: Enter saves to the highlighted one, Esc leaves it in “Unsorted” and clicking elsewhere discards it. The “Saved” notice has Undo and Move.",
    ],
  },
  {
    icon: "ask",
    title: "Asking",
    points: [
      "Click the squirrel → Ask. If you had text selected, it's used as context.",
      "With text selected, {quick} opens Summarize · Translate · Explain.",
      "Remove the tags on top (selection, nest) with their ✕.",
      "Below the answer: Copy, Paste in app, Save as acorn or open in Claude/ChatGPT.",
      "Needs an AI key (Settings → AI). Everything else works without one.",
    ],
  },
  {
    icon: "nest",
    title: "Nests (Kura)",
    points: [
      "Click the squirrel → Nests. Nests on the left, their acorns on the right.",
      "The search box searches everything you saved.",
      "Pin: marks the active nest (added as context when you ask).",
      "Pencil to rename (or double-click), colours, export to Markdown and delete.",
      "Open an acorn to read it, edit it (pencil), move it to another nest or delete it.",
      "A nest's + button creates a note you write yourself.",
    ],
  },
  {
    icon: "paste",
    title: "Paste nest",
    points: [
      "Click the squirrel → Paste nest → pick a nest.",
      "It's pasted as a clean block into the app you were in (Claude, ChatGPT, an email…).",
    ],
  },
  {
    icon: "acorn",
    title: "Quick acorn",
    points: [
      "Select text in any app (drag or double-click): a tiny acorn appears next to the cursor.",
      "Rest the pointer on it: Ask · Summarize · Translate · Explain · Save.",
      "If you keep working it goes away. It leaves your clipboard alone until you pick something.",
      "To turn it off or on: right-click the squirrel → “Quick acorn”, or Settings → Behaviour.",
    ],
  },
  {
    icon: "crop",
    title: "Ask about a capture",
    points: [
      "{capture_ask}: crop an area of the screen and ask about it right away (Describe · Get the text · Explain).",
      "After a normal capture, «Ask about it» does the same without saving it.",
      "In Kura, saved images have an «Ask» button. If a nest has images, the 3 newest go with your questions.",
      "No AI key: the image is copied and Claude or ChatGPT opens so you can paste it.",
    ],
  },
  {
    icon: "clipboard",
    title: "Clipboard history",
    points: [
      "It lives in Kura, below the nests (or press {clipboard}). It is optional: turn it on there or in Settings → Privacy.",
      "Click an entry to paste it where you were; you can also ask about it, save it as an acorn or delete it.",
      "Memory only: wiped when you quit. It never keeps passwords marked private, nor anything in private mode.",
    ],
  },
  {
    icon: "clock",
    title: "Pomodoro",
    points: [
      "Click the squirrel → Pomodoro. Pick Classic (25/5), Steady (50/10), Deep (90/20) or your own minutes.",
      "While you focus, her acorn fills with green; during breaks she naps.",
      "Every phase change shows a message next to her, never a sound.",
      "The menu shows the phase and time; the panel lets you pause, restart, skip, switch timer or stop.",
    ],
  },
  {
    icon: "plug",
    title: "Use your nests in Claude",
    points: [
      "Settings → Integrations → Connect to Claude Desktop (then restart Claude).",
      "Then, in Claude, type “use my Trip nest”. Works with your subscription, no key.",
    ],
  },
  {
    icon: "squirrel",
    title: "Moving, modes and hiding",
    points: [
      "Drag her to any edge: she sticks to it and remembers her spot on each screen.",
      "Lively: breathes and strolls. Still: only blinks. Hidden: only the acorn peeks out; click it to call her.",
      "Hide and bring back: {toggle}, or click the acorn in the system tray.",
      "She hides by herself in fullscreen and stays out of recordings (Settings → Behaviour).",
      "Right-click her: hide for 1 hour, until tomorrow or always in an app.",
    ],
  },
  {
    icon: "lock",
    title: "Privacy",
    points: [
      "Everything is stored on your computer; keys in the Windows keychain.",
      "Private mode (right-click → Private mode): saves nothing and wears a mask.",
      "It only goes online for your AI (when you ask) and, once a day, to check GitHub for a new version. You can turn the latter off in Settings → Privacy.",
    ],
  },
];

export const HELP = { es, en };
