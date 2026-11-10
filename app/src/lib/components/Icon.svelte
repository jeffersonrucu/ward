<script>
  // Icons (FigCraft frame 178): 20 px, stroke 1.5, color by the icon tone.
  import { ICONS } from './icons.js';

  /** @type {{ name: string, size?: 'xs' | 'sm' | 'md' | 'lg', tone?: string }} */
  let { name, size = 'md', tone = undefined } = $props();
  const paths = $derived(ICONS[name] ?? []);
</script>

<svg
  class="icon {size}"
  viewBox="0 0 20 20"
  fill="none"
  aria-hidden="true"
  focusable="false"
  data-icon-tone={tone}
>
  {#each paths as c, i (i)}
    <path
      transform={c.t}
      d={c.d}
      stroke="currentColor"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {/each}
</svg>

<style>
  .icon {
    flex: none;
    color: var(--icon-tone);
  }
  .xs {
    width: var(--icon-xs);
    height: var(--icon-xs);
  }
  .sm {
    width: var(--icon-sm);
    height: var(--icon-sm);
  }
  .md {
    width: var(--icon-md);
    height: var(--icon-md);
  }
  .lg {
    width: var(--icon-lg);
    height: var(--icon-lg);
  }
  path {
    stroke-width: var(--line-icon);
  }
</style>
