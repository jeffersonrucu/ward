<script>
  // App: with the vault unlocked, the shell with the side navigation and the chosen
  // screen; locked, the unlock screen (Phase 31), with the request of the agent when a
  // use waits for the unlock; otherwise, the skeleton of Phase 29 with the status of the
  // daemon. The M0 screens arrive in Phases 30–39, each from the nodes of
  // docs/mission/design-nodes.md.
  import { onDestroy, onMount } from 'svelte';
  import Button from './lib/components/Button.svelte';
  import Logo from './lib/components/Logo.svelte';
  import SideNav from './lib/components/SideNav.svelte';
  import Toast from './lib/components/Toast.svelte';
  import { call, listenNotices } from './lib/bridge/index.js';
  import Settings from './screens/Settings.svelte';
  import Credentials from './screens/Credentials.svelte';
  import Unlock from './screens/Unlock.svelte';
  import PasswordGenerator from './screens/GeradorSenha.svelte';
  import ApprovalWindow from './screens/JanelaAprovacao.svelte';
  import FirstUse from './screens/PrimeiroUso.svelte';

  // Keyed by the wire values of the vault status (frozen until Phase 54).
  const TEXTS = {
    'sem-cofre': 'Nenhum cofre criado ainda.',
    'travado': 'O cofre está travado.',
    'destravado': 'O cofre está destravado.',
  };
  const SCREENS = ['credenciais', 'gerador', 'configuracoes'];

  let message = $state('Conectando ao daemon…');
  let failed = $state(false);
  let unlocked = $state(false);
  let locked = $state(false);
  // Phase 31: the request of the agent that waits for the unlock: id, seconds and
  // (Phase 39a) the entry, the operation and the command with visible escape (D51). No
  // secret.
  let request = $state(null);
  // Phase 30: no vault, the first use; on create, the daemon leaves the vault unlocked.
  let noVault = $state(false);
  let screen = $state('credenciais');
  let Showcase = $state(null);
  // Phase 36: the approval window, opened by the Rust side when a request of the daemon
  // arrives, shows only the approval (its capability only has its own commands). The
  // label comes from the metadata Tauri injects in the window, with no IPC.
  let approval = $state(window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label === 'aprovacao');
  let stopListening = null;

  // Only the known fields, each one in the right type; the rest is ignored. The fields
  // are wire names (frozen until Phase 54).
  const text = (v) => (typeof v === 'string' ? v : '');
  function requestFrom(p) {
    return {
      id: p.id,
      restante: p.restante,
      entrada: text(p.entrada),
      operacao: text(p.operacao),
      comando: text(p.comando),
    };
  }

  function apply(r) {
    unlocked = r?.estado === 'destravado';
    locked = r?.estado === 'travado';
    noVault = r?.estado === 'sem-cofre';
    request = locked && r?.pedido ? requestFrom(r.pedido) : null;
    message = TEXTS[r?.estado] ?? 'Estado desconhecido.';
  }

  async function query() {
    failed = false;
    message = 'Conectando ao daemon…';
    try {
      apply(await call('estado_do_cofre'));
    } catch (e) {
      failed = true;
      locked = false;
      noVault = false;
      message = e.mensagem;
    }
  }

  // Notice from the Rust side (unlock request) or periodic query of the test bridge: the
  // status is read again without clearing the screen; an error here does not change the
  // screen.
  async function refresh() {
    if (failed) return;
    try {
      apply(await call('estado_do_cofre'));
    } catch {
      // The next action on the screen shows the error.
    }
  }

  function onCreated() {
    noVault = false;
    screen = 'credenciais';
    unlocked = true;
  }

  // The vault locked during use (Phase 35: 5th wrong PIN, S9): the shell goes away and
  // the status of the daemon is queried again, which leads to the default unlock screen.
  function onLocked() {
    unlocked = false;
    query();
  }

  // The lock now button of the side navigation (Phase 39): the Rust side clears the clipboard
  // and the form and locks; the shell goes away and the screen goes to unlock.
  async function lockNow() {
    try {
      await call('travar');
    } catch {
      // The status read again below shows what happened.
    }
    onLocked();
  }

  function unlockFailed(m) {
    failed = true;
    locked = false;
    message = m;
  }

  function navigate(next) {
    if (SCREENS.includes(next)) screen = next;
  }

  onMount(async () => {
    if (approval) return;
    // Only in the test build (gone from the production bundle): the opening URL of the
    // bridge may ask for a screen (a screen parameter in the URL) for the QA in Chrome, and the
    // component showcase.
    if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
      const { requestedScreen } = await import('./lib/bridge/test-bridge.js');
      const requested = requestedScreen();
      if (requested === 'aprovacao') {
        approval = true;
        return;
      }
      if (requested && SCREENS.includes(requested)) {
        screen = requested;
        unlocked = true;
        return;
      }
    }
    await query();
    stopListening = await listenNotices(refresh);
    if (import.meta.env.MODE === 'teste') {
      Showcase = (await import('./lib/test/Showcase.svelte')).default;
    }
  });
  onDestroy(() => stopListening?.());
</script>

{#if approval}
  <ApprovalWindow />
{:else if unlocked}
  <div class="shell">
    <SideNav active={screen} onnavigate={navigate} onlock={lockNow} />
    {#if screen === 'credenciais'}
      <Credentials onlocked={onLocked} />
    {:else if screen === 'gerador'}
      <PasswordGenerator />
    {:else}
      <Settings onlocked={onLocked} />
    {/if}
  </div>
{:else if locked}
  <Unlock {request} onunlocked={query} onerror={unlockFailed} />
{:else if noVault}
  <FirstUse oncreated={onCreated} />
{:else}
  <main class="window">
    <section class="panel">
      <Logo />
      <h1>Cofre de credenciais</h1>
      {#if failed}
        <Toast kind="error" {message} />
        <Button variant="secondary" onclick={query}>Tentar de novo</Button>
      {:else}
        <p class="status">{message}</p>
      {/if}
    </section>
    {#if Showcase}<Showcase />{/if}
  </main>
{/if}

<style>
  .shell {
    display: flex;
    gap: var(--space-16);
    height: 100%;
    padding: var(--space-16);
    background: var(--color-bg-default);
  }
  .window {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-32);
    min-height: 100%;
    padding: var(--space-48) var(--space-24);
    background: var(--color-bg-default);
  }
  .panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-20);
    width: var(--width-column-access);
    padding: var(--space-48);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  h1 {
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .status {
    margin: 0;
    color: var(--color-text-secondary);
  }
</style>
