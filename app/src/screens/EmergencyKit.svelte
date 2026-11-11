<script>
  // Printed emergency kit (FigCraft 16773, Phase 49; V7.10, D17).
  //
  // Opened from the emergency kit section of the settings screen ("Print"). Shows the A4
  // sheet (794×1123, page-a4 tokens, white paper and dark ink from the print colors) in a
  // window over the app, with "Close" and "Print". "Print" calls `window.print()`: the
  // system print dialog picks the printer, and a PDF only exists if the user picks
  // "Print to file" and the place. The app writes nothing to disk.
  //
  // The sheet has no secret: the folders are the fixed ones of the installation and from
  // the daemon comes only the external backup (the external backup command: on, UUID and
  // label of the USB drive, if connected; everything goes through `usbDriveLine`). The
  // master password is written by hand on the two blank lines; there is no field, value
  // or state for it.
  //
  // When printing (`@media print`), only the sheet comes out: the rest of the page goes
  // away. For that the <dialog> moves to the end of the <body> (outside `#app`) while it
  // is open.
  import Button from '../lib/components/Button.svelte';
  import Logo from '../lib/components/Logo.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';
  import {
    RESTORE_STEPS,
    BACKUPS_DIR,
    VAULT_DIR,
    printedDate,
    usbDriveLine,
  } from './emergency-kit.js';
  import { rustMessage } from './first-use.js';

  /** @type {{ open?: boolean, onclose?: () => void, onlocked?: () => void }} */
  let { open = false, onclose = undefined, onlocked = undefined } = $props();

  let dialog = $state();
  let external = $state(null);
  let ready = $state(false);
  let error = $state('');
  let date = $state(printedDate(new Date()));
  let readId = 0;

  const usbDrive = $derived(usbDriveLine(external));

  /** Moves the <dialog> to the end of the <body>, so printing hides everything else. */
  function toBody(node) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  async function load() {
    const mine = ++readId;
    ready = false;
    error = '';
    external = null;
    date = printedDate(new Date());
    try {
      // Phase 48: the external backup command brings on, UUID (D45) and the USB drive, if
      // connected (label already neutralized in the Rust side). No mount path, no secret.
      const b = await call('external_backup');
      if (mine !== readId) return;
      external = {
        enabled: b?.enabled === true,
        uuid: typeof b?.uuid === 'string' ? b.uuid : null,
        usbDrive: b?.usb_drive && typeof b.usb_drive.label === 'string' ? { label: b.usb_drive.label } : null,
      };
      ready = true;
    } catch (e) {
      if (mine !== readId) return;
      if (e?.code === 'locked') {
        close();
        onlocked?.();
        return;
      }
      error = rustMessage(e?.message) || 'Não foi possível ler o backup externo. Tente de novo.';
    }
  }

  function close() {
    readId++;
    if (dialog?.open) dialog.close();
    else onclose?.();
  }

  function print() {
    if (!ready) return;
    window.print();
  }

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      load();
    }
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  use:toBody
  class="kit"
  aria-labelledby="kit-title"
  onclose={() => {
    readId++;
    onclose?.();
  }}
>
  <header class="bar">
    <div class="texts">
      <h2 id="kit-title">Kit de emergência</h2>
      <p>
        Confira a folha e imprima. Para guardar em PDF, escolha imprimir em arquivo na janela de
        impressão; o Ward não grava a folha.
      </p>
    </div>
    <div class="actions">
      <Button variant="secondary" onclick={close}>Fechar</Button>
      <Button onclick={print} disabled={!ready} aria-label="Imprimir a folha">Imprimir</Button>
    </div>
  </header>
  {#if error}
    <div class="notice">
      <Toast kind="error" message={error} />
      <Button variant="secondary" onclick={load}>Tentar de novo</Button>
    </div>
  {/if}

  <div class="scroll">
    <article class="sheet" aria-label="Folha do kit de emergência" data-test="kit-sheet">
      <header class="header">
        <Logo print />
        <h1 class="title">Kit de emergência</h1>
        <p class="text">
          Guarde esta folha num lugar seguro, longe do computador. Com ela você encontra o cofre e
          os backups mesmo se esta máquina parar.
        </p>
        <p class="date">{date}</p>
      </header>

      <section class="section">
        <h2 class="title-section">Onde está o cofre</h2>
        <div class="item">
          <span class="label">Pasta do cofre nesta máquina</span>
          <span class="value mono">{VAULT_DIR}</span>
        </div>
      </section>

      <section class="section">
        <h2 class="title-section">Onde estão os backups</h2>
        <div class="item">
          <span class="label">Cópias locais · 30 versões, uma a cada mudança</span>
          <span class="value mono">{BACKUPS_DIR}</span>
        </div>
        <div class="item" data-test="kit-usb-drive">
          <span class="label">{usbDrive.label}</span>
          {#if ready}
            <span class="value" class:mono={usbDrive.mono}><bdi>{usbDrive.value}</bdi></span>
          {:else}
            <span class="value">Lendo o backup externo…</span>
          {/if}
        </div>
      </section>

      <section class="section">
        <h2 class="title-section">Senha mestra</h2>
        <p class="text">Escreva à mão. O Ward nunca preenche este espaço.</p>
        <div class="lines" aria-hidden="true">
          <div class="line"></div>
          <div class="line"></div>
        </div>
      </section>

      <section class="section">
        <h2 class="title-section">Como restaurar</h2>
        <ol class="steps">
          {#each RESTORE_STEPS as step, i (i)}
            <li>{step}</li>
          {/each}
        </ol>
      </section>

      <div class="spacer"></div>
      <p class="footer">
        O PIN de aprovação não aparece nesta folha: ele não abre o cofre. Depois de anotar a senha
        mestra, guarde a folha como guardaria a própria senha.
      </p>
    </article>
  </div>
</dialog>

<style>
  /* Window over the app (dark theme); the sheet is white paper. */
  .kit {
    width: 100%;
    height: 100%;
    max-width: none;
    max-height: none;
    margin: 0;
    padding: 0;
    border: none;
    background: var(--color-bg-default);
    color: var(--color-text-primary);
  }
  .kit[open] {
    display: flex;
    flex-direction: column;
  }
  .kit::backdrop {
    background: var(--color-bg-overlay);
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-24);
    padding: var(--space-16) var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .bar .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .bar h2 {
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .bar p {
    margin: 0;
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .actions {
    display: flex;
    gap: var(--space-12);
    flex: none;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-12) var(--space-24) var(--space-0);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-24);
  }

  /* A4 sheet (16773): 794×1123, padding space-64, gap space-32, print paper color. */
  .sheet {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    width: var(--page-a4-width);
    height: var(--page-a4-height);
    margin: 0 auto;
    padding: var(--space-64);
    overflow: hidden;
    background: var(--color-print-paper);
    color: var(--color-print-text);
    box-shadow: var(--shadow-modal);
    font-family: var(--font-family-text);
    print-color-adjust: exact;
    -webkit-print-color-adjust: exact;
  }
  .sheet h1,
  .sheet h2,
  .sheet p,
  .sheet ol {
    margin: 0;
  }
  /* header (16774): gap space-16, padding-bottom space-24, thin bottom border. */
  .header {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-16);
    padding-bottom: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-print-line);
  }
  /* Section title style (8:9). */
  .title {
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  /* Body text style (8:12) in the secondary print text color. */
  .text {
    color: var(--color-print-text-secondary);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  /* Caption text style (8:16). */
  .date,
  .footer {
    color: var(--color-print-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  /* section (16775–16778): gap space-12. */
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  /* Subsection title style (8:11). */
  .title-section {
    font-weight: var(--weight-semibold);
    font-size: var(--size-md);
    line-height: var(--line-height-md);
  }
  /* item (16790): gap space-4; label in the label text style (8:15), value in mono body (8:18). */
  .item {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .label {
    color: var(--color-print-text-secondary);
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .value {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    overflow-wrap: anywhere;
  }
  .value bdi {
    unicode-bidi: isolate;
  }
  .mono {
    font-family: var(--font-family-mono);
  }
  /* lines (16796): padding space-32 on top and space-8 below, gap space-40; each line is
     a thin border in the print line color (a border prints without a background). */
  .lines {
    display: flex;
    flex-direction: column;
    gap: var(--space-40);
    padding: var(--space-32) var(--space-0) var(--space-8);
  }
  .line {
    border-top: var(--line-thin) solid var(--color-print-line);
  }
  /* steps (16798): body text in the print text color, numbered "1." … "4.". */
  .steps {
    padding: var(--space-0);
    list-style-position: inside;
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .spacer {
    flex: 1;
  }

  /* Print: only the sheet, on an A4 page without margin (the sheet already has space-64). */
  @media print {
    @page {
      size: A4;
      margin: 0;
    }
    /* Only with the kit open: a Ctrl+P on another screen does not lose the page. */
    :global(html:has(> body > dialog.kit[open])),
    :global(body:has(> dialog.kit[open])) {
      height: auto;
      background: var(--color-print-paper);
    }
    :global(body:has(> dialog.kit[open]) > :not(dialog.kit)) {
      display: none !important;
    }
    .kit {
      position: static;
      width: auto;
      height: auto;
      overflow: visible;
      background: var(--color-print-paper);
    }
    .kit::backdrop {
      background: transparent;
    }
    .bar,
    .notice {
      display: none;
    }
    .scroll {
      overflow: visible;
      padding: 0;
    }
    .sheet {
      margin: 0;
      box-shadow: none;
      break-inside: avoid;
    }
  }
</style>
