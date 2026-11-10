<script>
  // Unlock screens: default (FigCraft 3342), wrong password (3393) and agent request
  // (3450). Window with the brand panel and the content panel (subtle background, subtle
  // border, panel radius, 48 padding) with the form column centered (access column
  // width; gap 32, or 24 with the request).
  //
  // Security (Phase 31; S1, S11, S13, D20):
  // - the password lives only in `password` (local state of this screen) and leaves it
  //   on the click, before the reply; it never goes to global state, browser storage or
  //   URL. A wrong password empties the field and keeps the focus on it (V5.8);
  // - the D20 wait is the daemon's, before it looks at the password. The button
  //   countdown ("try again in N s") is the same rule (doubles from 1 s, up to 30 s),
  //   counted here only with the errors of this screen: the reply does not say how many
  //   attempts there were;
  // - the agent request comes from the Rust side (the request of the vault status
  //   reply): id, seconds left by the daemon deadline and (Phase 39a) the requested
  //   entry, the operation and the exact command, already with visible escapes (D51: the
  //   daemon formats it and the app Rust neutralizes it). They come from the agent: they
  //   go in only as text (never HTML), isolated from the surrounding text direction and
  //   without ligatures. No type or environment: with the vault locked the daemon does
  //   not know them, so the card does not have the badges of frame 3450.
  import { onDestroy, onMount, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import CodeBox from '../lib/components/CodeBox.svelte';
  import AgentText from '../lib/components/AgentText.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import BrandPanel from '../lib/components/BrandPanel.svelte';
  import { call } from '../lib/bridge/index.js';

  /** Maximum D20 wait, in seconds. */
  const MAX_WAIT = 30;

  /** How the agent asked, by the protocol operation (its wire name). */
  const OPERATION_COMMAND = Object.freeze({
    'executar': 'ward run',
    ssh: 'ward ssh',
    login: 'ward login',
    playwright: 'ward playwright',
  });
  const operationCommand = (op) =>
    Object.hasOwn(OPERATION_COMMAND, op) ? OPERATION_COMMAND[op] : '';

  /**
   * @type {{
   *   request?: {
   *     id: number,
   *     restante: number,
   *     entrada: string,
   *     operacao: string,
   *     comando: string
   *   } | null,
   *   onunlocked: () => void,
   *   onerror?: (message: string) => void
   * }}
   */
  let { request = null, onunlocked, onerror = undefined } = $props();

  let password = $state('');
  let error = $state('');
  let consecutiveErrors = $state(0);
  let wait = $state(0);
  let sending = $state(false);
  /** @type {HTMLInputElement | null} */
  let input = $state(null);

  // Agent request: local countdown from the remaining seconds the Rust side sent; "not
  // now" hides only that request (by id).
  let remaining = $state(0);
  let requestId = $state(-1);
  let dismissed = $state(-1);
  $effect(() => {
    if (request && request.id !== requestId) {
      requestId = request.id;
      remaining = request.restante;
    } else if (request && request.id === requestId && Math.abs(request.restante - remaining) > 2) {
      remaining = request.restante;
    }
  });
  const withRequest = $derived(request !== null && remaining > 0 && dismissed !== requestId);

  let clock = null;
  onMount(() => {
    clock = setInterval(() => {
      if (wait > 0) wait -= 1;
      if (remaining > 0) remaining -= 1;
    }, 1000);
    input?.focus();
  });
  onDestroy(() => {
    clearInterval(clock);
    password = '';
  });

  function countdown(s) {
    const m = Math.floor(s / 60);
    const ss = String(s % 60).padStart(2, '0');
    return `${m}:${ss} restantes`;
  }

  async function unlock(ev) {
    ev?.preventDefault();
    if (sending || wait > 0) return;
    if (!password) {
      error = 'Digite a senha mestra.';
      input?.focus();
      return;
    }
    sending = true;
    // The password leaves the screen state before the reply.
    const sent = password;
    password = '';
    try {
      await call('destravar', { 'senha': sent });
      error = '';
      consecutiveErrors = 0;
      onunlocked();
    } catch (e) {
      if (e.codigo === 'senha-incorreta') {
        consecutiveErrors += 1;
        wait = Math.min(2 ** (consecutiveErrors - 1), MAX_WAIT);
        error = e.mensagem;
      } else if (e.codigo === 'travado' || e.codigo === 'pedido-invalido') {
        error = e.codigo === 'pedido-invalido' ? 'Senha mestra fora do formato.' : e.mensagem;
      } else {
        onerror?.(e.mensagem);
      }
    } finally {
      sending = false;
      await tick();
      input?.focus();
    }
  }

  function typed() {
    if (error && wait === 0) error = '';
  }

  const buttonLabel = $derived(
    sending ? 'Destravando…' : wait > 0 ? `Tente de novo em ${wait} s` : 'Destravar',
  );
</script>

<div class="window">
  <BrandPanel />
  <main class="content">
    <form class="form" class:with-request={withRequest} onsubmit={unlock} novalidate>
      <header class="header">
        {#if !withRequest}
          <div class="seal"><Icon name="lock" size="lg" tone="brand" /></div>
        {/if}
        <h1>Cofre travado</h1>
        <p class="description">
          {withRequest
            ? 'Um agente está esperando uma entrada. Destrave para continuar.'
            : 'Digite a senha mestra para destravar o Ward neste computador.'}
        </p>
      </header>

      {#if withRequest}
        <section class="request" aria-label="Pedido do agente">
          <div class="top">
            <span class="request-label">Pedido do agente</span>
            <span class="countdown" role="timer" aria-live="off">
              <Icon name="clock" size="xs" tone="warning" />{countdown(remaining)}
            </span>
          </div>
          <div class="entry">
            <span class="avatar"><Icon name="key" /></span>
            <div class="texts">
              <span class="name"><AgentText text={request.entrada} /></span>
              <span class="operation">{operationCommand(request.operacao)}</span>
            </div>
          </div>
          {#if request.comando}
            <CodeBox label="Comando"><AgentText text={request.comando} /></CodeBox>
          {/if}
        </section>
      {/if}

      <PasswordField
        id="master-password"
        label="Senha mestra"
        placeholder="Digite a senha mestra"
        help="Só a senha mestra destrava. O PIN não serve aqui."
        bind:value={password}
        bind:input={input}
        error={error}
        readonly={sending}
        autocomplete="off"
        oninput={typed}
      />

      {#if withRequest}
        <div class="actions">
          <Button variant="secondary" onclick={() => (dismissed = requestId)}>Agora não</Button>
          <Button type="submit" disabled={sending || wait > 0}>{buttonLabel}</Button>
        </div>
      {:else}
        <Button type="submit" disabled={sending || wait > 0}>{buttonLabel}</Button>
      {/if}

      {#if withRequest}
        <Highlight icon="alert">
          Destravar não aprova o pedido. Se a entrada pedir aprovação, a janela de aprovação
          abre em seguida, com PIN. “Agora não”, fechar ou deixar expirar nega o pedido.
        </Highlight>
      {:else if consecutiveErrors > 0}
        <Highlight icon="clock">
          Cada tentativa errada aumenta a espera da próxima, até 30 s. O Ward não mostra
          quantas tentativas já houve.
        </Highlight>
      {:else}
        <Highlight icon="clock">
          O cofre trava sozinho quando você bloqueia a tela, suspende o computador ou fica 4 h
          sem usar.
        </Highlight>
      {/if}
    </form>
  </main>
</div>

<style>
  .window {
    display: flex;
    gap: var(--space-16);
    height: 100%;
    padding: var(--space-16);
    background: var(--color-bg-default);
  }
  .content {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: var(--space-48);
    overflow-y: auto;
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    width: var(--width-column-access);
    margin: auto;
  }
  .form.with-request {
    gap: var(--space-24);
  }
  .form > :global(.button) {
    width: 100%;
  }
  .header {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  .seal {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-primary-subtle);
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .description {
    margin: var(--space-0);
    color: var(--color-text-secondary);
  }
  .request {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-default);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .request-label {
    font-weight: var(--weight-medium);
  }
  .countdown {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-8) var(--space-2) var(--space-6);
    border-radius: var(--radius-pill);
    background: var(--color-warning-subtle);
    color: var(--color-warning-text);
    font-weight: var(--weight-medium);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    font-variant-numeric: tabular-nums;
  }
  .entry {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .avatar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-md);
    height: var(--control-md);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-secondary);
  }
  .texts {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-primary);
    overflow-wrap: anywhere;
    /* D51: the name comes from the agent; each character as it is, isolated from the surrounding direction. */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
    direction: ltr;
    unicode-bidi: isolate;
  }
  .operation {
    font-family: var(--font-family-mono);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .actions {
    display: flex;
    gap: var(--space-12);
  }
  .actions > :global(.button) {
    flex: 1;
  }
</style>
