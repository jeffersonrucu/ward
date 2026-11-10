<script>
  // Approval window (Phase 36; S9, S10, D51). Approval frames normal (16701), prod
  // (16810), wrong PIN (16885), blocked PIN (16960) and expiring (17035): panel of 560
  // (width-wide-dialog), padding space-24, gap space-20, radius window, border
  // border-subtle, modal shadow.
  //
  // - The request comes from the Rust side (the approval request command) with the
  //   agent's text already neutralized (D51) and enters here only as text: no HTML. The
  //   window queries on open, on each approval request notice of the channel (the
  //   notice listener, the same as Phase 31, one per window), after answering and when
  //   the countdown reaches zero. The countdown runs here only for display; the daemon
  //   is the one that expires the request (S10). Nothing lives outside the component.
  // - Initial focus on "Deny"; Esc denies; Enter in the PIN allows nothing (V5.19).
  //   Closing (the X or the window) denies (S10). The PIN lives only in the state of the
  //   PIN field (FigCraft 650) and is cleared on each send.
  // - Only this window answers: in the app, the approval capability (the main one does
  //   not have the two commands) and the command checks the window label again.
  // - Phase 54b (D68): with a hidden stdin (the command's stdin is a pipe or a file), a
  //   warning highlight and a switch that enables the three "Allow" buttons; when on,
  //   the answer goes with the acknowledged flag. The Rust side and the daemon check it;
  //   the box unchecks on each new request. Frames 22964 (unchecked) and 23051
  //   (checked) in FigCraft (Phase 54f).
  import { onDestroy, onMount, tick } from 'svelte';
  import EnvironmentBadge from '../lib/components/EnvironmentBadge.svelte';
  import TypeBadge from '../lib/components/TypeBadge.svelte';
  import Button from '../lib/components/Button.svelte';
  import IconButton from '../lib/components/IconButton.svelte';
  import CodeBox from '../lib/components/CodeBox.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import Countdown from '../lib/components/Countdown.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import Switch from '../lib/components/Switch.svelte';
  import AgentText from '../lib/components/AgentText.svelte';
  import { entryType } from '../lib/components/types.js';
  import symbol from '../lib/components/ward-symbol.svg';
  import { call, listenNotices } from '../lib/bridge/index.js';
  import {
    DENY_ID,
    URGENT_SECONDS,
    SEPARATOR,
    TEXTS,
    originNotice,
    resultMessage,
    needsPin,
    pinSufficient,
  } from './approval.js';

  let request = $state(null);
  let loaded = $state(false);
  let pin = $state('');
  let error = $state('');
  let busy = $state(false);
  let failure = $state('');
  let acknowledged = $state(false);

  const blocked = $derived(request?.estado === 'bloqueado');
  const prod = $derived(request?.ambiente === 'prod');
  // Phase 44c (D9): only the confirmation of an `http` origin (no PIN, "Deny" or
  // "Confirm").
  const confirmation = $derived(request?.confirmacao === true);
  const popup = $derived(request?.operacao === 'preencher');
  // Phase 54b: the stdin that this window does not show.
  const hiddenStdin = $derived(request?.entrada_oculta === true);
  const notAcknowledged = $derived(hiddenStdin && !acknowledged);
  const urgent = $derived(!blocked && request !== null && request.restante <= URGENT_SECONDS);

  let querying = false;
  let clock = null;
  let stopListening = null;
  let left = false;

  async function focusDeny() {
    await tick();
    const b = document.getElementById(DENY_ID);
    if (b) b.focus({ focusVisible: true });
  }

  async function query() {
    if (querying || busy) return;
    querying = true;
    try {
      const fresh = await call('pedido_de_aprovacao');
      const other = (fresh?.id ?? null) !== (request?.id ?? null);
      request = fresh;
      failure = '';
      if (other) {
        pin = '';
        error = '';
        acknowledged = false;
        if (fresh) focusDeny();
      }
    } catch (e) {
      failure = e.mensagem;
    } finally {
      loaded = true;
      querying = false;
    }
  }

  async function answer(decision) {
    if (!request || busy) return;
    const id = request.id;
    const withPin = needsPin(decision);
    if (withPin && !pinSufficient(pin)) {
      error = TEXTS.pinMissing;
      return;
    }
    if (withPin && notAcknowledged) {
      error = TEXTS.stdinMissing;
      return;
    }
    const sent = withPin ? pin : null;
    const withAcknowledgment = withPin && hiddenStdin && acknowledged;
    pin = '';
    busy = true;
    try {
      const r = await call('responder_aprovacao', {
        id,
        'decisao': decision,
        pin: sent,
        'ciente': withAcknowledgment,
      });
      error = resultMessage(r?.resultado);
    } catch (e) {
      error = e.mensagem;
    } finally {
      busy = false;
    }
    await query();
  }

  function onKeyDown(e) {
    // V5.19: Esc denies. Enter has no shortcut: only the focused button answers.
    if (e.key === 'Escape') {
      e.preventDefault();
      answer('negar');
    }
  }

  // Display only: one second less; at zero, ask again (the expired request has already
  // left the Rust side).
  function tickDown() {
    if (!request || blocked) return;
    if (request.restante > 0) request.restante -= 1;
    if (request.restante === 0) query();
  }

  onMount(async () => {
    clock = setInterval(tickDown, 1000);
    await query();
    const stop = await listenNotices(query, 'pedido-de-aprovacao');
    if (left) stop();
    else stopListening = stop;
  });

  onDestroy(() => {
    left = true;
    if (clock) clearInterval(clock);
    if (stopListening) stopListening();
    pin = '';
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<main class="background">
  {#if request}
    <section class="approval" aria-labelledby="approval-title">
      <header class="header">
        <img class="symbol" src={symbol} alt="" />
        <div class="titles">
          <h1 id="approval-title">{TEXTS.title}</h1>
          <p class="subtitle">{popup ? TEXTS.popupSubtitle : TEXTS.subtitle}</p>
        </div>
        {#if !blocked}<Countdown seconds={request.restante} urgent={urgent} />{/if}
        <IconButton icon="close" label={TEXTS.closeAndDeny} onclick={() => answer('negar')} />
      </header>

      {#if blocked}
        <Highlight kind="danger" icon="lock">{TEXTS.blockedHighlight}</Highlight>
      {:else}
        {#if prod}<Highlight kind="prod"
            >{confirmation ? TEXTS.prodHighlightOrigin : TEXTS.prodHighlight}</Highlight
          >{/if}
        <!-- Phase 44e: the origin warning in a warning highlight (frames 22812, 22957). -->
        {#if request.origem}<Highlight kind="warning" icon="alert">{originNotice(request.origem)}</Highlight>{/if}
        <!-- Phase 54b (D68): the stdin that the window does not show (frames 22964, 23051). -->
        {#if hiddenStdin}<Highlight kind="warning" icon="alert">{TEXTS.stdinHighlight}</Highlight>{/if}
        {#if urgent}
          <Highlight kind="danger" icon="clock">{TEXTS.expiring(request.restante)}</Highlight>
        {/if}
      {/if}

      <p class="summary">
        {blocked
          ? TEXTS.blockedSummary
          : confirmation
            ? TEXTS.confirmationSummary
            : request.origem
              ? TEXTS.originSummary
              : TEXTS.summary}
      </p>

      <div class="request" class:prod>
        {#each request.entradas as entry, i (i)}
          <div class="entry">
            <span class="avatar"><Icon name={entryType(entry.tipo ?? 'generico').icon} /></span>
            <div class="texts">
              <span class="name"><AgentText text={entry.nome} /></span>
              <span class="badges">
                {#if entry.tipo}<TypeBadge kind={entry.tipo} />{/if}
                <EnvironmentBadge environment={request.ambiente} />
              </span>
            </div>
            {#if i === 0}
              <div class="level">
                <span class="level-label">{TEXTS.level}</span>
                <span class="level-value">{confirmation ? TEXTS.confirmationLevel : TEXTS.levelValue}</span>
              </div>
            {/if}
          </div>
        {/each}
        <!-- Phase 44c: the `http` origin where username and password go (the login, the
             playwright and the popup have no command). -->
        {#if request.origem}
          <CodeBox label={TEXTS.origin}><AgentText text={request.origem} /></CodeBox>
        {/if}
        {#if request.comando || !request.origem}
          <CodeBox label={TEXTS.command}><AgentText text={request.comando} /></CodeBox>
        {/if}
        <!-- Phase 26a: the command runs in the CLI's directory; only the run request has
             one (frames 16701, 16810, 16885, 16960, 17035, 22964; Phase 54i). -->
        {#if request.diretorio}
          <CodeBox label={TEXTS.directory}><AgentText text={request.diretorio} /></CodeBox>
        {/if}
        <!-- The arrow is only drawing (another color); one inside a name already arrives
             escaped. -->
        <CodeBox label={TEXTS.requestedBy}
          >{#if request.cadeia.length === 0}{TEXTS.emptyChain}{/if}{#each request.cadeia as process, i (i)}{#if i > 0}<span
                class="arrow"
                aria-hidden="true">{SEPARATOR}</span
              >{/if}<span class="process"><AgentText text={process} /></span>{/each}</CodeBox
        >
      </div>

      {#if confirmation}
        {#if error}<p class="failure" role="alert">{error}</p>{/if}
        <div class="actions">
          <div class="row">
            <Button id={DENY_ID} variant="secondary" disabled={busy} onclick={() => answer('negar')}>
              {TEXTS.deny}
            </Button>
            <Button variant="primary" disabled={busy || blocked} onclick={() => answer('confirmar')}>
              {TEXTS.confirm}
            </Button>
          </div>
        </div>
      {:else}
        {#if hiddenStdin && !blocked}
          <Switch
            id="approval-stdin"
            label={TEXTS.stdinAcknowledged}
            on={acknowledged}
            disabled={busy}
            onchange={(v) => (acknowledged = v)}
          />
        {/if}
        <PinField
          id="approval-pin"
          label={TEXTS.pin}
          bind:value={pin}
          help={TEXTS.pinHelp}
          error={error}
          blocked={blocked}
        />

        <div class="actions">
          <div class="row">
            <Button id={DENY_ID} variant="secondary" disabled={busy} onclick={() => answer('negar')}>
              {TEXTS.deny}
            </Button>
            <Button variant="primary" disabled={busy || blocked || notAcknowledged} onclick={() => answer('uma_vez')}>
              {TEXTS.once}
            </Button>
          </div>
          <div class="row">
            <Button variant="secondary" disabled={busy || blocked || notAcknowledged} onclick={() => answer('uma_hora')}>
              {TEXTS.oneHour}
            </Button>
            <Button variant="secondary" disabled={busy || blocked || notAcknowledged} onclick={() => answer('sempre')}>
              {TEXTS.always}
            </Button>
          </div>
        </div>
      {/if}

      <Highlight icon="alert"
        >{blocked ? TEXTS.blockedNote : confirmation ? TEXTS.confirmationNote : TEXTS.note}</Highlight
      >
      {#if failure}<p class="failure" role="alert">{failure}</p>{/if}
    </section>
  {:else if loaded}
    <section class="approval empty">
      <p class="summary">{failure || TEXTS.noRequest}</p>
    </section>
  {/if}
</main>

<style>
  .background {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    min-height: 100%;
    background: var(--color-bg-default);
  }
  .approval {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    width: var(--width-wide-dialog);
    max-width: 100%;
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-default);
    box-shadow: var(--shadow-modal);
  }
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .symbol {
    flex: none;
    width: var(--control-sm);
    height: var(--control-sm);
  }
  .titles {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
    color: var(--color-text-primary);
  }
  .subtitle {
    margin: var(--space-0);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .summary {
    margin: var(--space-0);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
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
  .request.prod {
    border-color: var(--color-environment-prod-border);
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
    /* D51: left to right, isolated; RTL letters isolated in the AgentText component. */
    direction: ltr;
    unicode-bidi: isolate;
  }
  .badges {
    display: flex;
    align-items: center;
    gap: var(--space-6);
  }
  .level {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-2);
  }
  .level-label {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .level-value {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-primary);
  }
  .arrow {
    color: var(--color-text-secondary);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-12);
  }
  .row :global(.button) {
    width: 100%;
  }
  .failure {
    margin: var(--space-0);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-danger-text);
  }
</style>
