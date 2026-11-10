<script>
  // Choose-type screen (FigCraft 17657; detail 17851): header with the plus avatar, the
  // title, the subtitle and the cancel button; a 3-column grid (gap space/12) with the 9
  // type cards (TypeCard); the level note at the end.
  // A card with `later` is disabled (Phase 34 brought the last types; none is now).
  import Button from '../lib/components/Button.svelte';
  import TypeCard from '../lib/components/TypeCard.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import { TYPE_CARDS } from './form.js';

  /** @type {{ onchoose: (kind: string) => void, oncancel: () => void }} */
  let { onchoose, oncancel } = $props();
</script>

<header class="header">
  <span class="avatar"><Icon name="plus" size="lg" /></span>
  <div class="identification">
    <h2 class="name">Nova credencial</h2>
    <p class="subtitle">Escolha o tipo. Ele não muda depois de salvo.</p>
  </div>
  <Button variant="secondary" onclick={oncancel}>Cancelar</Button>
</header>
<div class="body">
  <div class="grid" role="radiogroup" aria-label="Tipo da credencial">
    {#each TYPE_CARDS as c (c.kind)}
      <TypeCard
        kind={c.kind}
        name={c.name}
        description={c.description}
        disabled={!!c.later}
        onclick={() => onchoose(c.kind)}
      />
    {/each}
  </div>
  <p class="note">Toda entrada nova começa com aprovação. Mudar o nível depois pede o PIN.</p>
</div>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .identification {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
  }
  .name,
  .subtitle,
  .note {
    margin: 0;
  }
  .name {
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
  }
  .subtitle,
  .note {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
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
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-12);
  }
</style>
