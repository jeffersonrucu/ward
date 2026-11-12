<script>
  // Section title (FigCraft 6841): title in text-body-strong (14/500), description in
  // secondary text-caption (12), gap space-2; optional action on the right (ghost
  // button).

  /** @type {{ id: string, title: string, description?: string, action?: import('svelte').Snippet }} */
  let { id, title, description = '', action = undefined } = $props();
</script>

<div class="section">
  <div class="texts">
    <h3 class="title" {id}>{title}</h3>
    {#if description}<p class="description">{description}</p>{/if}
  </div>
  {#if action}{@render action()}{/if}
</div>

<style>
  .section {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
  }
  .title,
  .description {
    margin: 0;
  }
  .title {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .description {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
</style>
