<script>
  // Choice of the USB drive of the external backup (Phase 48, V7.9; D17). No frame of
  // its own in FigCraft: builds the modal (611) with the list of USB drives (options
  // drawn like the USB drive card 16318: default surface, item radius, 20 padding; the
  // chosen one with the focus border) and the buttons (263).
  //
  // The list comes from the daemon (the USB drive list command): UUID, label (from
  // udisks2, untrusted; it arrives with the invisible characters escaped, D51, and here
  // it is only text) and whether it is mounted. No mount path. On confirm, only the
  // chosen UUID goes; the Rust side checks again that it is in the daemon list before
  // writing.
  import Button from '../lib/components/Button.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import { call } from '../lib/bridge/index.js';
  import { usbDriveName, situationInList } from './settings.js';
  import { rustMessage } from './first-use.js';

  /**
   * @type {{ open?: boolean, replace?: boolean, current?: string | null,
   *   onclose?: () => void, onchosen?: (external: object) => void,
   *   onlocked?: () => void }}
   */
  let {
    open = false,
    replace = false,
    current = null,
    onclose = undefined,
    onchosen = undefined,
    onlocked = undefined,
  } = $props();

  let usbDrives = $state([]);
  let chosen = $state('');
  let loading = $state(false);
  let sending = $state(false);
  let failure = $state('');

  async function load() {
    loading = true;
    failure = '';
    try {
      const list = await call('list_usb_drives');
      usbDrives = Array.isArray(list) ? list : [];
      if (!usbDrives.some((p) => p.uuid === chosen)) {
        const same = usbDrives.find((p) => current && p.uuid.toUpperCase() === current.toUpperCase());
        chosen = same ? same.uuid : '';
      }
    } catch (e) {
      usbDrives = [];
      chosen = '';
      if (e?.code === 'locked') {
        onlocked?.();
        return;
      }
      failure = rustMessage(e?.message) || 'Não deu para ver os pendrives.';
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (open) {
      chosen = '';
      usbDrives = [];
      load();
    }
  });

  async function confirm() {
    if (!chosen || sending) return;
    sending = true;
    failure = '';
    try {
      const external = await call('set_external_backup', { 'enabled': true, uuid: chosen });
      onchosen?.(external);
    } catch (e) {
      if (e?.code === 'locked') {
        onlocked?.();
        return;
      }
      const msg = rustMessage(e?.message) || 'Não foi possível ligar o backup externo.';
      // The drive left (or the request did not match the list): the list comes back updated.
      if (e?.code === 'usb-drive-not-connected') await load();
      failure = msg;
    } finally {
      sending = false;
    }
  }
</script>

<Modal title={replace ? 'Trocar pendrive' : 'Escolher pendrive'} {open} onclose={() => onclose?.()}>
  <div class="content">
    <p class="explanation">
      O Ward copia as versões cifradas para a pasta ward-backup/ do pendrive escolhido sempre que
      ele estiver conectado e montado. Ele é reconhecido pelo UUID, não pelo nome.
    </p>
    {#if loading && usbDrives.length === 0}
      <p class="empty" role="status">Procurando pendrives…</p>
    {:else if usbDrives.length === 0 && !failure}
      <p class="empty" role="status">
        Nenhum pendrive conectado. Conecte o pendrive e toque em Atualizar.
      </p>
    {:else if usbDrives.length > 0}
      <div class="list" role="radiogroup" aria-label="Pendrives conectados">
        {#each usbDrives as p (p.uuid)}
          <label class="option" class:chosen={chosen === p.uuid}>
            <input
              class="visually-hidden"
              type="radio"
              name="pendrive"
              value={p.uuid}
              checked={chosen === p.uuid}
              onchange={() => (chosen = p.uuid)}
            />
            <span class="texts">
              <span class="name"><bdi>{usbDriveName(p)}</bdi></span>
              <span class="detail">UUID <bdi class="uuid">{p.uuid}</bdi> · {situationInList(p)}</span>
            </span>
          </label>
        {/each}
      </div>
    {/if}
    {#if failure}<p class="failure" role="alert">{failure}</p>{/if}
  </div>
  {#snippet footer()}
    <Button variant="ghost" onclick={load} disabled={loading || sending}>Atualizar</Button>
    <Button variant="secondary" onclick={() => onclose?.()}>Cancelar</Button>
    <Button onclick={confirm} disabled={!chosen || sending}
      >{replace ? 'Usar este pendrive' : 'Ligar'}</Button
    >
  {/snippet}
</Modal>

<style>
  .content {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }
  p {
    margin: 0;
  }
  .explanation,
  .empty {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-16) var(--space-20);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
    color: var(--color-text-primary);
    cursor: pointer;
  }
  .option.chosen {
    border-color: var(--color-border-focus);
  }
  .option:has(input:focus-visible) {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .name,
  .detail {
    overflow-wrap: anywhere;
  }
  bdi {
    unicode-bidi: isolate;
  }
  .uuid {
    direction: ltr;
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .detail {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .failure {
    color: var(--color-danger-text);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
</style>
