<script>
  // Credentials screen (Phase 32): frames 4780 (empty), 4063 (full), 4410 (filtered) and
  // 5259 (filters) of Ward.figcraft.
  //
  // Inside the shell of App.svelte (1280×800 window, padding and gap space/16, with the
  // side navigation): `main` (gap space/16) with `top` (padding space/4 space/8 0, gap
  // space/16: title and summary, pill search of width/search, the filter button) and
  // `columns` (gap space/16): `list` (width/column-list, padding and gap space/16 and
  // space/12) and `detail` (header, body and footer).
  //
  // S11/D4: the list and the detail come from Rust without secrets. The SecretField
  // draws the mask; reveal asks Rust for the field (D65 b) and the value leaves in
  // 30 s, on hide, on switching entries or on leaving; copy asks Rust, which puts the
  // value on the clipboard (the reply does not carry the value). Nothing here goes to
  // browser storage or the URL.
  //
  // Phase 33: the new-credential button opens ChooseType and the form in the detail
  // panel; edit opens the form of the entry. Phase 34: the 9 types. Phase 39b: an entry
  // with attachments can be edited too (the kept ones go by name; the content stays in
  // the daemon).
  //
  // Phase 35: the level card is LevelCard (FigCraft 18998) and its change button opens
  // LevelDialog (FigCraft 967), which asks for the PIN (S9). On change: reloads the list
  // and the detail. The 5th wrong PIN locks the vault: `onlocked` takes the App to the
  // locked screen.
  //
  // Phase 22d (D65 d): the footer of the detail (metadata, FigCraft 4400) shows the
  // created and changed dates from the daemon, in the local time zone; without a date,
  // nothing.
  //
  // Wire names (frozen until m2:Phase 54): the fields of the list items and of the entry
  // detail, and the arguments and replies of the Tauri commands.
  import { onMount, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import EnvironmentBadge from '../lib/components/EnvironmentBadge.svelte';
  import TypeBadge from '../lib/components/TypeBadge.svelte';
  import GroupHeader from '../lib/components/GroupHeader.svelte';
  import Field from '../lib/components/Field.svelte';
  import SecretField from '../lib/components/SecretField.svelte';
  import LevelCard from '../lib/components/LevelCard.svelte';
  import LevelDialog from '../lib/components/LevelDialog.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import FiltersDialog from '../lib/components/FiltersDialog.svelte';
  import ChooseType from './ChooseType.svelte';
  import Form from './Form.svelte';
  import { FORM_TYPES } from './form.js';
  import Icon from '../lib/components/Icon.svelte';
  import ListItem from '../lib/components/ListItem.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { TYPES, entryType } from '../lib/components/types.js';
  import { call } from '../lib/bridge/index.js';
  import {
    ENVIRONMENTS,
    EMPTY_FILTERS,
    groupItems,
    filterItems,
    activeFilters,
    groupsOf,
    topSummary,
    filtersSummary,
  } from './credentials.js';

  /** @type {{ onlocked?: () => void }} */
  let { onlocked = undefined } = $props();

  // How long the copied state stays on the field action.
  const COPIED_MS = 2000;
  const REVEAL_DEADLINE_MS = 30_000;
  const SUCCESS_MS = 4000;

  let items = $state([]);
  let loaded = $state(false);
  let error = $state('');
  let search = $state('');
  let filters = $state({ ...EMPTY_FILTERS });
  let dialogOpen = $state(false);
  let selected = $state('');
  let detail = $state(null);
  let copied = $state('');
  let searchInput = $state();
  let listEl = $state();
  let scroll = $state({ fade: false });
  /** 'detail' | 'choose' | 'form' */
  let mode = $state('detail');
  let formKind = $state('');
  let formDetail = $state(null);
  let success = $state('');
  /** Revealed value of a secret of the detail (one at a time). */
  let revealed = $state({ id: '', value: '' });
  let revealDeadline = 0;
  let successDeadline = 0;
  let levelOpen = $state(false);

  const filtering = $derived(activeFilters(filters) > 0 || search.trim() !== '');
  const visible = $derived(filterItems(items, filters, search));
  const groups = $derived(groupItems(items, visible, filtering));
  const order = $derived(groups.flatMap((g) => g.items.map((i) => i.nome)));
  const empty = $derived(loaded && items.length === 0);
  const summary = $derived(
    topSummary(items.length, groupsOf(items).length, visible.length, filtering),
  );
  const nFilters = $derived(activeFilters(filters));

  /** `dd/mm/yyyy` in the local time zone, from UTC seconds (D65 d). */
  function date(seconds) {
    if (!Number.isSafeInteger(seconds) || seconds <= 0) return '';
    const d = new Date(seconds * 1000);
    if (Number.isNaN(d.getTime())) return '';
    const two = (n) => String(n).padStart(2, '0');
    return `${two(d.getDate())}/${two(d.getMonth() + 1)}/${d.getFullYear()}`;
  }
  const metadata = $derived.by(() => {
    if (!detail) return '';
    const created = date(detail.criada_em);
    const changed = date(detail.alterada_em);
    return [created && `Criada em ${created}`, changed && `Alterada em ${changed}`]
      .filter(Boolean)
      .join(' · ');
  });

  const groupOptions = $derived([
    { value: '', label: 'Todos' },
    ...groupsOf(items).map((g) => ({ value: g.name, label: g.name })),
  ]);
  const kindOptions = [
    { value: '', label: 'Todos' },
    ...Object.entries(TYPES).map(([value, t]) => ({ value, label: t.label })),
  ];
  const environmentOptions = [
    { value: '', label: 'Todos' },
    ...ENVIRONMENTS.map((a) => ({ value: a, label: a })),
  ];

  async function load() {
    error = '';
    try {
      items = await call('listar_credenciais');
      loaded = true;
    } catch (e) {
      error = e.mensagem;
    }
  }

  // Only the reply of the last detail request counts (fast clicks).
  let detailRequest = 0;

  async function openEntry(name, focus = true) {
    selected = name;
    copied = '';
    hide();
    mode = 'detail';
    const mine = ++detailRequest;
    try {
      const d = await call('detalhe_credencial', { 'entrada': name });
      if (mine === detailRequest) detail = d;
    } catch (e) {
      if (mine === detailRequest) {
        detail = null;
        error = e.mensagem;
      }
    }
    if (focus) {
      await tick();
      itemOf(name)?.focus();
    }
  }

  let timer = null;

  async function copy(field) {
    if (!detail) return;
    error = '';
    try {
      await call('copiar_campo', { 'entrada': detail.nome, 'campo': field });
      copied = field;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = ''), COPIED_MS);
    } catch (e) {
      error = e.mensagem;
    }
  }

  function hide() {
    clearTimeout(revealDeadline);
    revealed = { id: '', value: '' };
  }

  async function reveal(field) {
    if (!detail) return;
    const entry = detail.nome;
    try {
      const r = await call('revelar_campo', { 'entrada': entry, 'campo': field });
      if (detail?.nome !== entry || mode !== 'detail') return;
      revealed = { id: field, value: r.valor };
      clearTimeout(revealDeadline);
      revealDeadline = setTimeout(hide, REVEAL_DEADLINE_MS);
    } catch (e) {
      error = e.mensagem;
    }
  }

  const editable = $derived(!!detail && Object.hasOwn(FORM_TYPES, detail.tipo));

  function newCredential() {
    hide();
    error = '';
    mode = 'choose';
  }

  function edit() {
    if (!editable) return;
    hide();
    error = '';
    formKind = detail.tipo;
    formDetail = detail;
    mode = 'form';
  }

  function cancelForm() {
    mode = 'detail';
  }

  async function saved(name) {
    mode = 'detail';
    await load();
    await openEntry(name, false);
    success = 'Credencial salva.';
    clearTimeout(successDeadline);
    successDeadline = setTimeout(() => (success = ''), SUCCESS_MS);
  }

  function changeLevel() {
    if (!detail) return;
    hide();
    error = '';
    levelOpen = true;
  }

  async function levelChanged() {
    const name = detail?.nome;
    levelOpen = false;
    if (!name) return;
    await load();
    await openEntry(name, false);
    success = 'Nível de acesso alterado.';
    clearTimeout(successDeadline);
    successDeadline = setTimeout(() => (success = ''), SUCCESS_MS);
  }

  function vaultLocked() {
    levelOpen = false;
    hide();
    detail = null;
    items = [];
    onlocked?.();
  }

  function itemOf(name) {
    return listEl?.querySelector(`[data-entry="${CSS.escape(name)}"]`);
  }

  // Arrows move between the visible items (focus only); Enter (or a click) opens.
  function keyInList(e) {
    const current = e.target?.closest?.('[data-entry]')?.dataset.entry;
    if (current === undefined) return;
    const k = order.indexOf(current);
    let target = null;
    if (e.key === 'ArrowDown') target = order[Math.min(k + 1, order.length - 1)];
    else if (e.key === 'ArrowUp') target = order[Math.max(k - 1, 0)];
    else if (e.key === 'Home') target = order[0];
    else if (e.key === 'End') target = order[order.length - 1];
    if (target !== null && target !== undefined) {
      e.preventDefault();
      itemOf(target)?.focus();
      itemOf(target)?.scrollIntoView({ block: 'nearest' });
    }
  }

  function keyInSearch(e) {
    if (e.key === 'ArrowDown' && order.length > 0) {
      e.preventDefault();
      itemOf(order[0])?.focus();
    } else if (e.key === 'Enter' && order.length > 0) {
      e.preventDefault();
      openEntry(order[0]);
    } else if (e.key === 'Escape' && search) {
      e.preventDefault();
      search = '';
    }
  }

  // Search shortcut: Ctrl+K anywhere, or "/" outside a text field.
  function globalKey(e) {
    if (dialogOpen || levelOpen || empty) return;
    const inField = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
    if ((e.ctrlKey && (e.key === 'k' || e.key === 'K')) || (e.key === '/' && !inField)) {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    }
  }

  function clearAll() {
    filters = { ...EMPTY_FILTERS };
    search = '';
  }

  function applyFilters(f) {
    filters = { group: f.group, kind: f.kind, environment: f.environment };
    dialogOpen = false;
  }

  function measureScroll() {
    if (!listEl) return;
    const { scrollTop, scrollHeight, clientHeight } = listEl;
    scroll = { fade: scrollTop + clientHeight < scrollHeight - 1 };
  }

  // With new filters, opens the first visible entry if the selected one is gone.
  $effect(() => {
    if (!loaded) return;
    if (order.length === 0) {
      selected = '';
      detail = null;
    } else if (!order.includes(selected)) {
      openEntry(order[0], false);
    }
  });

  $effect(() => {
    void order;
    tick().then(measureScroll);
  });

  onMount(() => {
    load();
    return () => {
      clearTimeout(timer);
      clearTimeout(successDeadline);
      hide();
    };
  });
</script>

<svelte:window onkeydown={globalKey} onresize={measureScroll} />

<main class="main" aria-labelledby="credentials-title">
  <header class="top">
    <div class="title">
      <h1 id="credentials-title">Credenciais</h1>
      <p class="summary">{summary}</p>
    </div>
    <div class="search">
      <Field
        id="search"
        label="Buscar"
        hiddenLabel
        pill
        icon="search"
        placeholder="Buscar por nome, host ou usuário"
        bind:value={search}
        bind:input={searchInput}
        disabled={empty}
        type="search"
        aria-keyshortcuts="Control+K /"
        onkeydown={keyInSearch}
      />
    </div>
    <Button
      variant="secondary"
      icon="filter"
      disabled={empty}
      aria-haspopup="dialog"
      onclick={() => (dialogOpen = true)}
    >
      {nFilters > 0 ? `Filtros · ${nFilters}` : 'Filtrar'}
    </Button>
  </header>

  <div class="columns">
    <section class="list" aria-label="Entradas">
      {#if filtering && !empty}
        <div class="active-filters">
          <span class="filters-summary">{filtersSummary(filters, search)}</span>
          <Button variant="ghost" onclick={clearAll}>Limpar</Button>
        </div>
      {/if}
      <div class="scroll">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="items"
          class:center={empty || (loaded && order.length === 0)}
          bind:this={listEl}
          onscroll={measureScroll}
          onkeydown={keyInList}
        >
          {#if empty}
            <p class="empty-list">As entradas aparecem aqui, separadas por grupo.</p>
          {:else if loaded && order.length === 0}
            <p class="empty-list">Nenhuma entrada com essa busca ou esses filtros.</p>
          {:else}
            {#each groups as g (g.name)}
              <div class="group" role="group" aria-labelledby="group-{g.color}-{g.name}">
                <GroupHeader
                  id="group-{g.color}-{g.name}"
                  group={g.name}
                  count={g.count}
                  color={g.color}
                />
                {#each g.items as item (item.nome)}
                  <ListItem
                    compact
                    name={item.nome}
                    kind={item.tipo}
                    environment={item.ambiente}
                    detail={item.resumo}
                    selected={item.nome === selected}
                    tabindex={item.nome === selected ? 0 : -1}
                    data-entry={item.nome}
                    onclick={() => openEntry(item.nome)}
                  />
                {/each}
              </div>
            {/each}
          {/if}
        </div>
        {#if scroll.fade}<div class="fade" aria-hidden="true"></div>{/if}
      </div>
      {#if error}<Toast kind="error" message={error} />{/if}
      {#if success}<Toast kind="success" message={success} />{/if}
      <div class="new">
        <Button variant="primary" icon="plus" onclick={newCredential}>Nova credencial</Button>
      </div>
    </section>

    <section class="detail" aria-label="Detalhe da entrada" aria-live="polite">
      {#if mode === 'choose'}
        <ChooseType
          onchoose={(t) => {
            formKind = t;
            formDetail = null;
            mode = 'form';
          }}
          oncancel={cancelForm}
        />
      {:else if mode === 'form'}
        {#key formDetail?.nome ?? formKind}
          <Form
            kind={formKind}
            detail={formDetail}
            groups={groupsOf(items).map((g) => g.name)}
            servers={items.filter((i) => i.tipo === 'server').map((i) => i.nome)}
            oncancel={cancelForm}
            onsaved={saved}
          />
        {/key}
      {:else if empty}
        <div class="body center">
          <div class="empty">
            <span class="avatar"><Icon name="key" size="lg" /></span>
            <h2 class="empty-title">Nenhuma credencial ainda</h2>
            <p class="empty-text">
              Crie a primeira entrada em Nova credencial. Cada entrada tem um tipo, um grupo e um
              ambiente (dev, stage ou prod).
            </p>
            <Highlight icon="lock">
              Entradas novas começam com o nível com aprovação: cada uso do Claude abre a janela
              de aprovação, que pede o seu PIN. Você pode mudar o nível depois, também com o PIN.
            </Highlight>
          </div>
        </div>
      {:else if detail}
        <header class="header">
          <span class="avatar"><Icon name={entryType(detail.tipo).icon} size="lg" /></span>
          <div class="identification">
            <h2 class="name">{detail.nome}</h2>
            <div class="badges">
              <TypeBadge kind={detail.tipo} />
              <EnvironmentBadge environment={detail.ambiente} />
              <span class="subtitle">{detail.subtitulo}</span>
            </div>
          </div>
          <Button
            variant="secondary"
            disabled={!editable}
            onclick={edit}>Editar</Button
          >
        </header>
        <div class="body">
          {#if detail.campos.length > 0}
            <div class="grid">
              {#each detail.campos as c (c.id)}
                <Field
                  id="field-{c.id}"
                  label={c.rotulo}
                  value={c.valor}
                  readOnly
                  copied={copied === c.id}
                  oncopy={() => copy(c.id)}
                />
              {/each}
            </div>
          {/if}
          {#each detail.segredos as s (s.id)}
            <SecretField
              id="secret-{s.id}"
              label={s.rotulo}
              revealed={revealed.id === s.id ? revealed.value : null}
              copied={copied === s.id}
              onreveal={() => reveal(s.id)}
              onhide={hide}
              oncopy={() => copy(s.id)}
            />
          {/each}
          {#if detail.extras.length > 0}
            <div class="grid">
              {#each detail.extras as x (x.nome)}
                {#if x.segredo}
                  <SecretField
                    id="extra-{x.nome}"
                    label={x.nome}
                    help=""
                    revealed={revealed.id === x.nome ? revealed.value : null}
                    copied={copied === x.nome}
                    onreveal={() => reveal(x.nome)}
                    onhide={hide}
                    oncopy={() => copy(x.nome)}
                  />
                {:else}
                  <Field
                    id="extra-{x.nome}"
                    label={x.nome}
                    value={x.valor ?? ''}
                    readOnly
                    copied={copied === x.nome}
                    oncopy={() => copy(x.nome)}
                  />
                {/if}
              {/each}
            </div>
          {/if}
          <LevelCard level={detail.nivel} onchange={changeLevel} />
        </div>
        <footer class="footer">
          <span class="metadata">{metadata}</span>
          <Button variant="secondary" icon="copy">Duplicar</Button>
          <Button variant="danger">Excluir</Button>
        </footer>
      {:else if loaded && order.length === 0}
        <div class="body center">
          <div class="empty">
            <span class="avatar"><Icon name="search" size="lg" /></span>
            <h2 class="empty-title">Nenhuma entrada encontrada</h2>
            <p class="empty-text">Limpe a busca ou os filtros para ver todas as entradas.</p>
          </div>
        </div>
      {/if}
    </section>
  </div>
</main>

<FiltersDialog
  open={dialogOpen}
  filters={{ group: filters.group, kind: filters.kind, environment: filters.environment }}
  groups={groupOptions}
  kinds={kindOptions}
  environments={environmentOptions}
  onapply={applyFilters}
  onclose={() => (dialogOpen = false)}
/>

<LevelDialog
  open={levelOpen}
  entry={detail}
  onclose={() => (levelOpen = false)}
  onchanged={levelChanged}
  onlocked={vaultLocked}
/>

<style>
  .main {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-4) var(--space-8) var(--space-0);
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .summary {
    margin: var(--space-0);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .search {
    width: var(--width-search);
  }
  .columns {
    display: flex;
    gap: var(--space-16);
    flex: 1;
    min-height: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    flex: none;
    width: var(--width-column-list);
    padding: var(--space-16);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .active-filters {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  .filters-summary {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .scroll {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    height: 100%;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .items.center {
    justify-content: center;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .empty-list {
    margin: var(--space-0);
    text-align: center;
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  /* Scroll fade (FigCraft 18948): gradient bg/subtle-transparent → bg/subtle. */
  .fade {
    position: absolute;
    left: var(--space-0);
    right: var(--space-0);
    bottom: var(--space-0);
    height: var(--space-48);
    background: linear-gradient(var(--color-bg-subtle-transparent), var(--color-bg-subtle));
    pointer-events: none;
  }
  .new {
    display: flex;
  }
  .new :global(.button) {
    flex: 1;
  }
  .detail {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow: hidden;
  }
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .identification {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    flex: 1;
    min-width: 0;
  }
  .name {
    margin: var(--space-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .badges {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }
  .subtitle {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    min-height: 0;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .body.center {
    align-items: center;
    justify-content: center;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    column-gap: var(--space-16);
    row-gap: var(--space-20);
  }
  .grid :global(.field) {
    min-width: 0;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16) var(--space-24);
    border-top: var(--line-thin) solid var(--color-border-subtle);
  }
  .metadata {
    flex: 1;
    min-width: var(--space-0);
    color: var(--color-text-secondary);
    font-weight: var(--weight-regular);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    letter-spacing: var(--letter-spacing-normal);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-16);
    width: var(--width-column-access);
    text-align: center;
  }
  .empty-title {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .empty-text {
    margin: var(--space-0);
    color: var(--color-text-secondary);
  }
  .empty :global(.highlight) {
    align-self: stretch;
    text-align: left;
  }
</style>
