<script>
  // Settings / General screen (FigCraft 15242, Phase 38; V5.22).
  //
  // Two columns: access (15289: master password, approval PIN and lock timeout, the
  // three option rows) and backup (15325: local backup, external backup on a USB drive,
  // emergency kit and the "backups always encrypted" note).
  //
  // - Changing the master password asks for the current one; changing the PIN asks for
  //   the current PIN (V5.22). Both dialogs keep what is typed only in their local state
  //   and clear it before the reply.
  // - Lock timeout: read from the daemon (the vault config, D30) and written by it (new
  //   generation, D39). Default 4 h.
  // - External backup (Phase 48, V7.9; FigCraft 16178): off by default (D17). Turning it
  //   on opens the USB drive choice (`ChooseUsbDrive`); when on, the USB drive card
  //   (16318) shows the chosen one and the change button. Turning it off keeps the stored
  //   UUID and deletes nothing from the drive. Read by the external backup command (no
  //   mount path). Restore (Phase 47, V7.7) opens the restore dialog (16349/16512).
  // - Phase 49a: an unsafe local backup folder (the daemon writes the vault with no
  //   backup) becomes a warning highlight right below the local backup row; only a yes/no
  //   from the daemon, no path.
  // - Emergency kit (Phase 49, V7.10): the print button opens the sheet.
  //
  // No secret value goes through this screen: the replies carry only the minutes and
  // the external backup (on, USB drive UUID, label already neutralized in Rust and
  // whether it is mounted). Nothing goes to storage, URL or global state.
  import { onDestroy, onMount } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import UsbDriveCard from '../lib/components/UsbDriveCard.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import OptionRow from '../lib/components/OptionRow.svelte';
  import Select from '../lib/components/Select.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';
  import EmergencyKit from './KitEmergencia.svelte';
  import ChangePin from './ChangePin.svelte';
  import ChangePassword from './ChangePassword.svelte';
  import ChooseUsbDrive from './ChooseUsbDrive.svelte';
  import RestoreBackup from './RestaurarBackup.svelte';
  import { mensagemRestaurado as restoredMessage } from './restaurar.js';
  import { LOCAL_BACKUP_WARNING, usbDriveCard, lockOptions } from './settings.js';
  import { mensagemDoRust as rustMessage } from './primeiro-uso.js';

  /** @type {{ onlocked?: () => void }} */
  let { onlocked = undefined } = $props();

  const SUCCESS_MS = 4_000;

  let config = $state(null);
  let minutes = $state('');
  let error = $state('');
  let success = $state('');
  let dialog = $state('');
  let saving = $state(false);
  let external = $state(null);
  let changingExternal = $state(false);
  let successTimer = null;

  const options = $derived(lockOptions(config?.lockMinutes));
  const externalOn = $derived(external ? external.ligado === true : config?.externalBackup === true);
  const card = $derived(usbDriveCard(external));

  function notify(text) {
    error = '';
    success = text;
    clearTimeout(successTimer);
    successTimer = setTimeout(() => (success = ''), SUCCESS_MS);
  }

  function apply(c) {
    config = {
      lockMinutes: c.minutos_trava,
      externalBackup: c.backup_externo === true,
      localBackup: c.backup_local !== false,
    };
    minutes = String(config.lockMinutes);
  }

  /** Error of a request: locked goes to the unlock screen; the rest shows in the toast. */
  async function failed(e, fallback) {
    if (e?.codigo === 'travado') {
      onlocked?.();
      return;
    }
    success = '';
    error = rustMessage(e?.mensagem) || fallback;
    await checkLock();
  }

  /** A write that fails after being checked can lock the vault in the daemon (D53). */
  async function checkLock() {
    try {
      const r = await call('estado_do_cofre');
      if (r?.estado === 'destravado') return;
    } catch {
      // No reply: the unlock screen asks again.
    }
    onlocked?.();
  }

  async function load() {
    try {
      apply(await call('configuracoes'));
      external = await call('backup_externo');
    } catch (e) {
      await failed(e, 'Não foi possível ler as configurações.');
    }
  }

  /** Turning on opens the USB drive choice; turning off writes right away (the UUID stays stored). */
  async function changeExternal(enable) {
    if (!external || changingExternal) return;
    if (enable) {
      dialog = 'usb-drive';
      return;
    }
    changingExternal = true;
    try {
      external = await call('definir_backup_externo', { 'ligado': false, uuid: null });
      config = config && { ...config, externalBackup: false };
      notify('Backup externo desligado. As cópias no pendrive continuam lá.');
    } catch (e) {
      await failed(e, 'Não foi possível desligar o backup externo.');
    } finally {
      changingExternal = false;
    }
  }

  function usbDriveChosen(chosen) {
    const replaced = external?.ligado === true;
    dialog = '';
    external = chosen;
    config = config && { ...config, externalBackup: chosen?.ligado === true };
    notify(replaced ? 'Pendrive do backup externo trocado.' : 'Backup externo ligado.');
  }

  async function changeLock(value) {
    const next = Number(value);
    if (!config || saving || next === config.lockMinutes) return;
    saving = true;
    try {
      apply(await call('definir_tempo_trava', { 'minutos': next }));
      notify('Tempo de trava salvo.');
    } catch (e) {
      minutes = String(config.lockMinutes);
      await failed(e, 'Não foi possível salvar o tempo de trava.');
    } finally {
      saving = false;
    }
  }

  function closeDialog() {
    dialog = '';
  }

  onMount(load);
  onDestroy(() => clearTimeout(successTimer));
</script>

<section class="main" aria-labelledby="settings-title">
  <header class="top">
    <h1 id="settings-title" class="title-page">Configurações</h1>
    <p class="summary">Acesso ao cofre, trava automática e backup.</p>
  </header>

  <div class="columns">
    <section class="access" aria-labelledby="access-title">
      <h2 id="access-title" class="title-section">Acesso</h2>
      <OptionRow id="config-password" title="Senha mestra">
        {#snippet description()}Só ela destrava o cofre. Trocar pede a senha atual.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialog = 'password')} aria-label="Trocar senha mestra"
            >Trocar</Button
          >
        {/snippet}
      </OptionRow>
      <OptionRow id="config-pin" title="PIN de aprovação">
        {#snippet description()}4 a 6 dígitos. Aprova usos, não destrava. Trocar pede o PIN atual.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialog = 'pin')} aria-label="Trocar PIN de aprovação"
            >Trocar</Button
          >
        {/snippet}
      </OptionRow>
      <OptionRow id="config-lock" title="Tempo de trava">
        {#snippet description()}Sem uso, trava após este tempo (máximo 4 h). Também trava ao bloquear a
          tela e ao suspender.{/snippet}
        {#snippet control()}
          <Select
            label="Tempo de trava"
            id="config-lock-timeout"
            compact
            bind:value={minutes}
            options={options}
            disabled={!config || saving}
            onchange={changeLock}
          />
        {/snippet}
      </OptionRow>
    </section>

    <section class="backup" aria-labelledby="backup-title">
      <h2 id="backup-title" class="title-section">Backup</h2>
      <OptionRow id="config-local-backup" title="Backup local">
        {#snippet description()}Automático e cifrado a cada mudança, com 30 versões em
          ~/.local/share/ward/backups/.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialog = 'restore')} aria-label="Restaurar backup"
            >Restaurar</Button
          >
        {/snippet}
      </OptionRow>
      {#if config && !config.localBackup}
        <div id="config-local-backup-warning" role="status">
          <Highlight kind="warning" icon="alert">{LOCAL_BACKUP_WARNING}</Highlight>
        </div>
      {/if}
      <OptionRow
        id="config-external-backup"
        title="Backup externo em pendrive"
        on={externalOn}
        disabled={!external || changingExternal}
        onchange={changeExternal}
      >
        {#snippet description()}{#if externalOn}Ligado. O Ward copia as versões para o pendrive
            escolhido sempre que ele for conectado, também com 30 versões.{:else}Desligado. Ao
            ligar, escolha o pendrive; o Ward copia as versões sempre que ele for conectado.{/if}{/snippet}
      </OptionRow>
      {#if external?.ligado && external.uuid}
        <UsbDriveCard
          name={card.name}
          uuid={external.uuid}
          situation={card.situation}
          status={card.status}
          mounted={card.mounted}
          disabled={changingExternal}
          onchange={() => (dialog = 'usb-drive')}
        />
      {/if}
      <OptionRow id="config-kit" title="Kit de emergência">
        {#snippet description()}Folha para imprimir com onde ficam o cofre e os backups e um espaço
          para anotar a senha mestra à mão.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialog = 'kit')} aria-label="Imprimir kit de emergência"
            >Imprimir</Button
          >
        {/snippet}
      </OptionRow>
      <div class="note">
        <Icon name="lock" />
        <div class="texts">
          <span class="note-title">Backups sempre cifrados</span>
          <span class="note-text">
            Cada versão fica cifrada com a senha mestra que valia quando foi gravada. Restaurar
            pede essa senha e guarda o cofre atual como backup antes da troca.
          </span>
        </div>
      </div>
      <div class="notices">
        {#if error}<Toast kind="error" message={error} />{/if}
        {#if success}<Toast kind="success" message={success} />{/if}
      </div>
    </section>
  </div>
</section>

<EmergencyKit
  aberto={dialog === 'kit'}
  onfechar={closeDialog}
  ontravado={() => {
    dialog = '';
    onlocked?.();
  }}
/>
<ChangePassword
  open={dialog === 'password'}
  onclose={closeDialog}
  onchanged={() => {
    dialog = '';
    notify('Senha mestra trocada.');
  }}
  onfailure={() => checkLock()}
/>
<ChooseUsbDrive
  open={dialog === 'usb-drive'}
  replace={external?.ligado === true}
  current={external?.uuid ?? null}
  onclose={closeDialog}
  onchosen={usbDriveChosen}
  onlocked={() => {
    dialog = '';
    onlocked?.();
  }}
/>
<RestoreBackup
  aberto={dialog === 'restore'}
  onfechar={closeDialog}
  onrestaurado={(r) => {
    dialog = '';
    notify(restoredMessage(r.instante, r.entradas));
    load();
  }}
  ontravado={() => {
    dialog = '';
    onlocked?.();
  }}
/>
<ChangePin
  open={dialog === 'pin'}
  onclose={closeDialog}
  onchanged={() => {
    dialog = '';
    notify('PIN trocado.');
  }}
  onlocked={() => {
    dialog = '';
    onlocked?.();
  }}
  onfailure={() => checkLock()}
/>

<style>
  .main {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    flex: 1;
    min-width: 0;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-8) var(--space-0);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .summary {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .columns {
    display: flex;
    gap: var(--space-16);
    flex: 1;
    min-height: 0;
  }
  .access,
  .backup {
    display: flex;
    flex-direction: column;
    padding: var(--space-24);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow-y: auto;
  }
  .access {
    gap: var(--space-12);
    flex: none;
    width: var(--width-column-list);
  }
  .backup {
    gap: var(--space-20);
    flex: 1;
    min-width: 0;
  }
  .note {
    display: flex;
    gap: var(--space-12);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  .note-title {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .note-text {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .notices {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
</style>
