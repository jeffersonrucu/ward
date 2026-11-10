<script>
  // Restore backup from the settings screen (Phase 47, V7.7; D17, D39, S9). Frames of
  // the versions step (16349, dialog 16458) and of the confirm step (16512, dialog
  // 16621), the restore dialog (set 13033): width width-wide-dialog, `hug`, radius
  // window, padding space-24, gap space-20, raised surface background, default border,
  // modal shadow. Header gap space-8: texts (gap space-4: modal title, secondary small
  // text subtitle) and the close icon.
  // - Versions: list with gap space-8 with the version items (11264), the caption
  //   (secondary caption text) and the footer on the right (gap space-12): Cancel and
  //   Continue.
  // - Confirm: content with gap space-16 with the chosen item, the password field
  //   (17502) for the master password of this version, the PIN field (650) for the
  //   current PIN (the frame does not have it; the daemon requires the PIN of now,
  //   Phase 45) and the warning highlight (18684); the caption and the footer with Back
  //   on the left and "Restore this version" on the right.
  //
  // The versions come from the daemon with only the name, the instant and two marks (no
  // size, origin, generation or content): the bottom line is "Local copy" (Phase 49a:
  // "of another master password", or as a warning "write older than the date says" for
  // the old copy with a new name). The version's password and the PIN live only in the
  // local state of this component, leave it before the answer of the request and on
  // closing or going back. Nothing goes to storage, the URL or a log.
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import VersionItem from '../lib/components/VersionItem.svelte';
  import { call } from '../lib/bridge/index.js';
  import { rustMessage } from './first-use.js';
  import {
    versionDetail,
    swapWarning,
    versionDate,
    versionDay,
    versionsCaption,
    howLongAgo,
  } from './restore.js';

  /**
   * @type {{ open?: boolean, onclose?: () => void,
   *   onrestored?: (r: { instant: number, entries: number }) => void,
   *   onlocked?: () => void }}
   */
  let { open = false, onclose = undefined, onrestored = undefined, onlocked = undefined } =
    $props();

  const PASSWORD_ID = 'restore-password';
  const PIN_ID = 'restore-pin';

  let dialog = $state();
  /** 'versions' | 'confirm' */
  let step = $state('versions');
  let versions = $state([]);
  let chosen = $state('');
  let loading = $state(false);
  let password = $state('');
  let pin = $state('');
  let passwordError = $state('');
  let pinError = $state('');
  let failure = $state('');
  let sending = $state(false);
  let now = $state(new Date());

  const version = $derived(versions.find((v) => v.nome === chosen) ?? null);

  function clear() {
    password = '';
    pin = '';
  }

  function reset() {
    clear();
    step = 'versions';
    chosen = '';
    versions = [];
    passwordError = '';
    pinError = '';
    failure = '';
    sending = false;
  }

  async function load() {
    loading = true;
    failure = '';
    now = new Date();
    try {
      const list = await call('listar_backups');
      versions = Array.isArray(list) ? list : [];
      if (!versions.some((v) => v.nome === chosen)) chosen = '';
    } catch (e) {
      versions = [];
      chosen = '';
      if (e?.codigo === 'travado') {
        onlocked?.();
        return;
      }
      failure = rustMessage(e?.mensagem) || 'Não foi possível ler as versões do backup.';
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      reset();
      dialog.showModal();
      load();
    }
    if (!open && dialog.open) dialog.close();
  });

  onDestroy(clear);

  function close() {
    clear();
    onclose?.();
  }

  async function focusOn(id) {
    await tick();
    document.getElementById(id)?.focus();
  }

  function next() {
    if (!version) return;
    clear();
    passwordError = '';
    pinError = '';
    failure = '';
    step = 'confirm';
    focusOn(PASSWORD_ID);
  }

  function back() {
    clear();
    passwordError = '';
    pinError = '';
    failure = '';
    step = 'versions';
  }

  async function restore(e) {
    e?.preventDefault();
    if (sending || !version) return;
    passwordError = '';
    pinError = '';
    failure = '';
    if (!password) {
      passwordError = 'Digite a senha mestra desta versão.';
      return focusOn(PASSWORD_ID);
    }
    if (pin.length < 4) {
      pinError = 'O PIN tem de 4 a 6 dígitos.';
      return focusOn(PIN_ID);
    }
    // The password and the PIN leave the state before the answer: the screen does not
    // keep them, not even in the error.
    const chosenNow = version;
    const args = { 'dados': { 'versao': chosenNow.nome, 'senha': password, pin } };
    clear();
    sending = true;
    try {
      const r = await call('restaurar_backup', args);
      args.dados = null;
      sending = false;
      onrestored?.({ instant: chosenNow.instante, entries: r?.entradas ?? 0 });
      return;
    } catch (err) {
      args.dados = null;
      sending = false;
      const code = err?.codigo ?? '';
      if (code === 'senha-incorreta') {
        passwordError = 'Essa senha não abre esta versão. Nada mudou no cofre atual.';
        return focusOn(PASSWORD_ID);
      }
      if (code === 'pin-incorreto') {
        pinError = 'PIN incorreto. Nada foi alterado.';
        if (await locked()) return;
        return focusOn(PIN_ID);
      }
      if (code === 'travado' || code === 'cofre-mudou') {
        onlocked?.();
        return;
      }
      if (code === 'backup-inexistente') {
        // The version left the folder (rotation or another process): the list comes back
        // updated.
        step = 'versions';
        await load();
        failure = rustMessage(err?.mensagem);
        return;
      }
      failure = rustMessage(err?.mensagem) || 'Não foi possível restaurar esta versão.';
      await locked();
    }
  }

  /** After an error: the 5th wrong PIN (or a failure to write) locks the vault in the
   * daemon. */
  async function locked() {
    try {
      const r = await call('estado_do_cofre');
      if (r?.estado === 'destravado') return false;
    } catch {
      // No answer: treat it as locked; the unlock screen queries again.
    }
    onlocked?.();
    return true;
  }
</script>

<dialog
  bind:this={dialog}
  class="dialog"
  aria-labelledby="restore-title"
  aria-describedby="restore-subtitle"
  data-test="restore-backup"
  data-step={step}
  onclose={close}
>
  <div class="header">
    <div class="texts">
      <h2 id="restore-title">{step === 'versions' ? 'Restaurar backup' : 'Confirmar restauração'}</h2>
      <p id="restore-subtitle" class="subtitle">
        {#if step === 'versions'}
          Escolha a versão que volta a ser o cofre. O cofre atual é guardado como backup antes da
          troca.
        {:else}
          Digite a senha mestra que valia nesta versão. Ela pode ser diferente da atual.
        {/if}
      </p>
    </div>
    <button class="close" type="button" aria-label="Fechar" onclick={close}>
      <Icon name="close" />
    </button>
  </div>

  {#if step === 'versions'}
    {#if loading && versions.length === 0}
      <p class="empty" role="status">Procurando versões…</p>
    {:else if versions.length === 0 && !failure}
      <p class="empty" role="status">
        Ainda não há versões. O Ward guarda uma a cada mudança no cofre.
      </p>
    {:else if versions.length > 0}
      <div class="versions" role="radiogroup" aria-label="Versões do backup">
        {#each versions as v, i (v.nome)}
          <VersionItem
            id="restore-version-{i}"
            value={v.nome}
            date={versionDate(v.instante)}
            detail={versionDetail(v)}
            alert={v.fora_de_ordem === true}
            when={howLongAgo(v.instante, now)}
            checked={chosen === v.nome}
            onchoose={(n) => (chosen = n)}
          />
        {/each}
      </div>
      <p class="caption">{versionsCaption(versions.length)}</p>
    {/if}
    {#if failure}<p class="failure" role="alert">{failure}</p>{/if}
    <div class="footer">
      <Button variant="secondary" onclick={close}>Cancelar</Button>
      <Button variant="primary" disabled={!version} onclick={next}>Continuar</Button>
    </div>
  {:else if version}
    <form class="content" onsubmit={restore} novalidate>
      <VersionItem
        id="restore-chosen"
        staticView
        date={versionDate(version.instante)}
        detail={versionDetail(version)}
        alert={version.fora_de_ordem === true}
        when={howLongAgo(version.instante, now)}
      />
      <PasswordField
        label="Senha mestra desta versão"
        id={PASSWORD_ID}
        bind:value={password}
        help="É a senha que valia em {versionDay(version.instante)}."
        error={passwordError}
      />
      <PinField
        label="PIN atual"
        id={PIN_ID}
        bind:value={pin}
        help="O PIN de agora confirma a troca. 5 erros seguidos travam o cofre."
        error={pinError}
      />
      <Highlight kind="warning" icon="alert">{swapWarning(version.instante)}</Highlight>
      <button class="visually-hidden" type="submit" tabindex="-1">Restaurar esta versão</button>
    </form>
    <p class="caption">Se a senha não abrir esta versão, nada muda no cofre atual.</p>
    {#if failure}<p class="failure" role="alert">{failure}</p>{/if}
    <div class="footer split">
      <Button variant="secondary" onclick={back}>Voltar</Button>
      <Button variant="primary" disabled={sending} onclick={restore}
        >Restaurar esta versão</Button
      >
    </div>
  {/if}
</dialog>

<style>
  .dialog {
    width: var(--width-wide-dialog);
    max-width: calc(100vw - 2 * var(--space-24));
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-modal);
  }
  .dialog[open] {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
  }
  .dialog::backdrop {
    background: var(--color-bg-overlay);
  }
  .header {
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  p {
    margin: var(--space-0);
  }
  .subtitle,
  .empty {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .close {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icon-md);
    height: var(--icon-md);
    padding: var(--space-0);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
  }
  .close:focus-visible {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
  /* Five items in view, as in the frame (74 px each: padding, date, gap, detail and
     border); the older ones scroll. */
  .versions {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    max-height: calc(
      5 *
        (
          2 * var(--space-16) + var(--line-height-base) + var(--space-2) +
            var(--line-height-sm) + 2 * var(--line-thin)
        ) +
        4 * var(--space-8)
    );
    overflow-y: auto;
  }
  .content {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    margin: var(--space-0);
  }
  .caption {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .failure {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-danger-text);
  }
  .footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-12);
  }
  .footer.split {
    justify-content: space-between;
  }
</style>
