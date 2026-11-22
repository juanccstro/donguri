<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { closeSelf, fitWindow, loadConfig, t, type Tail } from "$lib/app.svelte";
  import Beak from "$lib/Beak.svelte";
  import Icon from "$lib/Icon.svelte";
  import { markdown } from "$lib/md";

  interface Nest {
    id: number;
    name: string;
    count: number;
  }
  interface AskContext {
    selection: string | null;
    words: number;
    quick: boolean;
    nest: Nest | null;
    has_key: boolean;
    /** Started from the selection acorn with an action already chosen. */
    auto: "summarize" | "translate" | "explain" | null;
    /** Without a key, questions open here. */
    handoff: "claude" | "chatgpt";
    /** A capture or saved image to ask about (data URL). */
    image: string | null;
  }
  interface Msg {
    role: "user" | "assistant";
    content: string;
  }

  let ctx = $state<AskContext | null>(null);
  let useSelection = $state(true);
  let useNest = $state(true);
  let useImage = $state(true);
  let messages = $state<Msg[]>([]);
  let shown = $state(""); // the answer as typed so far
  let queue = ""; // received but not yet typed
  let streaming = $state(false);
  let error = $state<string | null>(null);
  let noKey = $state(false);
  let trimmed = $state(false);
  let model = $state("");
  let question = $state("");
  let copied = $state(false);
  let tail = $state<Tail>({ edge: null, x: 0, y: 0, space: [0, 0, 0, 0] } as Tail);
  let box: HTMLElement;
  let input = $state<HTMLTextAreaElement>();
  let answerEl = $state<HTMLElement>();
  let chatId = 0;
  let typer: number | undefined;
  let waited = $state(0); // seconds without text yet
  let waitTimer: number | undefined;
  let switched = $state<string | null>(null);

  const lastAnswer = $derived(
    [...messages].reverse().find((m) => m.role === "assistant")?.content ?? "",
  );
  const hasSelection = $derived(!!ctx?.selection && useSelection);
  const hasImage = $derived(!!ctx?.image && useImage);
  const firstQuestion = $derived(messages.find((m) => m.role === "user")?.content ?? "");

  // Text arrives in bursts; type it out letter by letter.
  function startTyping() {
    clearInterval(typer);
    typer = setInterval(async () => {
      if (!queue) {
        if (!streaming) clearInterval(typer);
        return;
      }
      const n = Math.max(2, Math.ceil(queue.length / 25));
      shown += queue.slice(0, n);
      queue = queue.slice(n);
      await tick();
      if (answerEl) answerEl.scrollTop = answerEl.scrollHeight;
    }, 16);
  }

  /** No API key: the question (with its context) opens in the user's own
   *  Claude or ChatGPT, using their subscription. */
  async function handoff(text: string, target = ctx?.handoff ?? "claude") {
    text = text.trim();
    if (!text) return;
    await invoke("handoff", {
      question: text,
      selection: hasSelection ? ctx?.selection : null,
      nestId: useNest ? (ctx?.nest?.id ?? null) : null,
      target,
      image: hasImage,
    });
    closeSelf();
  }

  function startWaiting() {
    waited = 0;
    clearInterval(waitTimer);
    waitTimer = setInterval(() => {
      if (shown || !streaming) clearInterval(waitTimer);
      else waited++;
    }, 1000);
  }

  async function send(text: string) {
    text = text.trim();
    if (!text || streaming) return;
    if (!ctx?.has_key) return handoff(text);
    error = null;
    switched = null;
    if (shown) messages.push({ role: "assistant", content: shown + queue });
    shown = "";
    queue = "";
    messages.push({ role: "user", content: text });
    question = "";
    streaming = true;
    chatId = Date.now();
    startTyping();
    startWaiting();
    try {
      const meta = await invoke<{ provider: string; model: string; truncated: boolean }>("chat_start", {
        req: {
          id: chatId,
          messages: $state.snapshot(messages),
          selection: hasSelection ? ctx?.selection : null,
          nest_id: useNest ? (ctx?.nest?.id ?? null) : null,
          image: hasImage,
        },
      });
      model = meta.model;
      trimmed = meta.truncated;
    } catch (e) {
      streaming = false;
      const msg = String(e);
      if (msg.includes("NO_KEY")) noKey = true;
      else error = msg;
      clearInterval(waitTimer);
    }
  }

  function quickImage(kind: "describe" | "read_text" | "explain") {
    send(t(kind === "describe" ? "p_describe" : kind === "read_text" ? "p_read_text" : "p_explain_image"));
  }

  function quick(kind: "summarize" | "translate" | "explain") {
    const prompt =
      kind === "translate"
        ? t("p_translate", { lang: t("lang_name") })
        : t(kind === "summarize" ? "p_summarize" : "p_explain");
    send(prompt);
  }

  // Without a key the bubble still works: it hands the question over.
  $effect(() => {
    if (ctx) noKey = !ctx.has_key;
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send(question);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  function stop() {
    invoke("chat_stop", { id: chatId });
    streaming = false;
    shown += queue;
    queue = "";
  }

  function close() {
    if (streaming) invoke("chat_stop", { id: chatId });
    closeSelf();
  }

  const answerText = () => (shown + queue) || lastAnswer;

  async function copy() {
    await invoke("copy_text", { text: answerText() });
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  onMount(() => {
    loadConfig();
    invoke<AskContext>("ask_context").then(async (c) => {
      ctx = c;
      await tick();
      if (c.auto) quick(c.auto);
      else input?.focus();
    });
    const off = listen<{ id: number; kind: "delta" | "done" | "error" | "switch"; text: string }>("chat", (e) => {
      if (e.payload.id !== chatId) return;
      if (e.payload.kind === "delta") queue += e.payload.text;
      else if (e.payload.kind === "switch") {
        switched = e.payload.text;
        waited = 0;
      } else if (e.payload.kind === "done") {
        streaming = false;
        model = e.payload.text || model;
        tick().then(() => input?.focus());
      } else {
        streaming = false;
        error = e.payload.text;
      }
    });
    const stopFit = fitWindow(box, (tl) => (tail = tl));
    return () => {
      off.then((f) => f());
      stopFit();
      clearInterval(typer);
      clearInterval(waitTimer);
    };
  });
</script>

<div class="wrap" bind:this={box} data-tail={tail.edge}>
  <div class="bubble px-box">
    {#if tail.edge}
      <Beak
        edge={tail.edge}
        offset={tail.edge === "bottom" || tail.edge === "top"
          ? Math.min(Math.max(tail.x - 6, 18), 342)
          : Math.min(Math.max(tail.y - 6, 18), 260)}
      />
    {/if}

    <header>
      <span class="title">Donguri</span>
      {#if model}<span class="muted model">{model}</span>{/if}
      <span class="grow"></span>
      <button class="btn ghost" title={t("minimize")} onclick={() => invoke("bubble_hide")}><Icon name="minus" size={14} /></button>
      <button class="btn ghost" title={t("close")} onclick={close}><Icon name="close" size={14} /></button>
    </header>

    {#if ctx?.image && useImage}
      <div class="shot">
        <img src={ctx.image} alt={t("using_image")} />
        <button class="btn ghost" title={t("close")} onclick={() => (useImage = false)}><Icon name="close" size={11} /></button>
      </div>
    {/if}

    {#if ctx && ((ctx.selection && useSelection) || (ctx.nest && useNest))}
      <div class="tags">
        {#if ctx.selection && useSelection}
          <span class="chip">
            <Icon name="paste" size={12} />{t(ctx.words === 1 ? "using_sel_1" : "using_sel", { n: ctx.words })}
            <button title={t("close")} onclick={() => (useSelection = false)}><Icon name="close" size={11} /></button>
          </span>
        {/if}
        {#if ctx.nest && useNest}
          <span class="chip">
            <Icon name="acorn" size={12} />{t("nest_tag", { name: ctx.nest.name })}
            <button title={t("close")} onclick={() => (useNest = false)}><Icon name="close" size={11} /></button>
          </span>
        {/if}
      </div>
    {/if}

    {#if noKey}
      <!-- No API key: questions open in the user's own Claude / ChatGPT. -->
      <p class="hint muted">
        {t("handoff_hint")}
        <button class="link" onclick={() => invoke("open_window", { name: "ajustes:ia" })}>{t("setup_ai")}</button>
      </p>
    {/if}
    {#if true}
      {#if messages.length || shown}
        <div class="answer scroll" bind:this={answerEl}>
          {#each messages as m, i}
            {#if m.role === "user"}
              {#if i > 0 || messages.length > 1}<div class="q">{m.content}</div>{/if}
            {:else}
              <div class="md">{@html markdown(m.content)}</div>
            {/if}
          {/each}
          {#if shown}
            <div class="md">{@html markdown(shown)}{#if streaming}<span class="caret"></span>{/if}</div>
          {:else if streaming}
            <div class="waiting">
              <div class="dots"><span></span><span></span><span></span></div>
              {#if waited >= 3}<span class="muted small">{t("waiting_s", { n: waited })}</span>{/if}
            </div>
            {#if waited >= 8}<p class="muted small">{t("waiting_slow")}</p>{/if}
          {/if}
        </div>
      {/if}

      {#if switched}<div class="note muted"><Icon name="warn" size={12} />{t("model_switched", { change: switched })}</div>{/if}
      {#if trimmed}<div class="note warn"><Icon name="warn" size={12} />{t("nest_trimmed")}</div>{/if}
      {#if error}<div class="note warn"><Icon name="warn" size={12} />{t("error_prefix")} {error}</div>{/if}

      {#if !messages.length && hasImage}
        <div class="quick">
          <button class="btn" onclick={() => quickImage("describe")}>{t("describe")}</button>
          <button class="btn" onclick={() => quickImage("read_text")}>{t("read_text")}</button>
          <button class="btn" onclick={() => quickImage("explain")}>{t("explain")}</button>
        </div>
      {:else if !messages.length && hasSelection}
        <div class="quick">
          <button class="btn" onclick={() => quick("summarize")}>{t("summarize")}</button>
          <button class="btn" onclick={() => quick("translate")}>{t("translate")}</button>
          <button class="btn" onclick={() => quick("explain")}>{t("explain")}</button>
        </div>
      {/if}

      {#if lastAnswer || (shown && !streaming)}
        {#if !streaming}
          <div class="actions">
            <button class="btn" onclick={copy}><Icon name="copy" size={13} />{copied ? t("copied") : t("copy")}</button>
            <button class="btn" onclick={() => invoke("paste_text", { text: answerText() })}><Icon name="paste" size={13} />{t("paste_app")}</button>
            <button class="btn" onclick={() => invoke("save_answer", { text: answerText(), title: firstQuestion.slice(0, 60) })}><Icon name="acorn" size={13} />{t("save_acorn")}</button>
            <button class="btn" onclick={() => invoke("open_in", { target: "claude", text: `${firstQuestion}\n\n${answerText()}` })}><Icon name="out" size={13} />Claude</button>
            <button class="btn" onclick={() => invoke("open_in", { target: "chatgpt", text: `${firstQuestion}\n\n${answerText()}` })}><Icon name="out" size={13} />ChatGPT</button>
          </div>
        {/if}
      {/if}

      <div class="ask">
        <textarea
          class="field"
          aria-label={messages.length ? t("follow_ph") : t("ask_ph")}
          rows="1"
          bind:this={input}
          bind:value={question}
          onkeydown={onKey}
          placeholder={messages.length ? t("follow_ph") : t("ask_ph")}
          oninput={(e) => {
            const el = e.currentTarget;
            el.style.height = "auto";
            el.style.height = `${Math.min(el.scrollHeight, 96)}px`;
          }}
        ></textarea>
        {#if streaming}
          <button class="btn" title={t("stop")} onclick={stop}><Icon name="close" size={14} /></button>
        {:else if noKey}
          <button class="btn primary" title={t("open_claude")} disabled={!question.trim()} onclick={() => handoff(question, "claude")}>Claude</button>
          <button class="btn" title={t("open_chatgpt")} disabled={!question.trim()} onclick={() => handoff(question, "chatgpt")}>ChatGPT</button>
        {:else}
          <button class="btn primary" title={t("send")} disabled={!question.trim()} onclick={() => send(question)}>
            <Icon name="send" size={14} />
          </button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .shot {
    position: relative;
    margin: 0 0 8px;
    display: flex;
    justify-content: center;
    background: var(--sunk);
    padding: 4px;
  }
  .shot img {
    display: block;
    max-width: 100%;
    max-height: 150px;
    object-fit: contain;
  }
  .shot button {
    position: absolute;
    top: 4px;
    right: 4px;
    padding: 2px;
  }
  .wrap {
    display: inline-block;
    width: 380px;
    padding: 6px;
  }
  .wrap[data-tail="bottom"] { padding-bottom: 16px; }
  .wrap[data-tail="top"] { padding-top: 16px; }
  .wrap[data-tail="left"] { padding-left: 16px; }
  .wrap[data-tail="right"] { padding-right: 16px; }

  .bubble {
    padding: 8px 10px 10px;
    max-width: 360px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .grow {
    flex: 1;
  }

  .model {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 150px;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 4px 0 8px;
  }

  .answer {
    max-height: 280px;
    margin: 4px 0 8px;
    padding-right: 4px;
    user-select: text;
  }

  .q {
    color: var(--ink-soft);
    font-size: 12px;
    border-left: 3px solid var(--terracotta-light);
    padding-left: 6px;
    margin: 8px 0 4px;
  }

  .md :global(p) {
    margin: 0 0 6px;
  }
  .md :global(ul),
  .md :global(ol) {
    margin: 0 0 6px;
    padding-left: 18px;
  }
  .md :global(h4) {
    font-family: var(--title);
    font-size: 12px;
    margin: 8px 0 4px;
  }
  .md :global(code) {
    background: var(--sunk);
    padding: 0 3px;
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
  .md :global(pre) {
    background: var(--sunk);
    padding: 6px;
    overflow-x: auto;
    box-shadow: inset 3px 0 0 var(--terracotta);
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
  }

  .caret {
    display: inline-block;
    width: 7px;
    height: 12px;
    background: var(--accent);
    margin-left: 2px;
    animation: blink 0.8s steps(2) infinite;
  }

  .dots {
    display: flex;
    gap: 4px;
    padding: 6px 0;
  }
  .dots span {
    width: 6px;
    height: 6px;
    background: var(--accent);
    animation: hop 0.9s steps(3) infinite;
  }
  .dots span:nth-child(2) { animation-delay: 0.15s; }
  .dots span:nth-child(3) { animation-delay: 0.3s; }

  .note {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    margin: 2px 0 6px;
  }
  .warn {
    color: var(--warn);
  }

  .quick,
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 8px;
  }

  .actions .btn {
    font-size: 12px;
    padding: 3px 7px;
  }

  .ask {
    display: flex;
    align-items: flex-end;
    gap: 6px;
  }

  textarea {
    resize: none;
    min-height: 30px;
    max-height: 96px;
    line-height: 1.3;
  }

  .hint {
    font-size: 12px;
    margin: 2px 0 8px;
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
    cursor: pointer;
    font-size: inherit;
  }

  .waiting {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .small {
    font-size: 12px;
  }


  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  @keyframes hop {
    50% {
      transform: translateY(-4px);
    }
  }
</style>
