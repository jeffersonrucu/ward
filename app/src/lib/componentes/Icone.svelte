<script>
  // `Ícone/*` (frame 178): 20 px, traço 1,5, cor pelo `Tom do ícone`.
  import { ICONES } from './icones.js';

  /** @type {{ nome: string, tamanho?: 'xs' | 'sm' | 'md' | 'lg', tom?: string }} */
  let { nome, tamanho = 'md', tom = undefined } = $props();
  const caminhos = $derived(ICONES[nome] ?? []);
</script>

<svg
  class="icone {tamanho}"
  viewBox="0 0 20 20"
  fill="none"
  aria-hidden="true"
  focusable="false"
  data-tom-icone={tom}
>
  {#each caminhos as c, i (i)}
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
  .icone {
    flex: none;
    color: var(--tom-icone);
  }
  .xs {
    width: var(--icone-xs);
    height: var(--icone-xs);
  }
  .sm {
    width: var(--icone-sm);
    height: var(--icone-sm);
  }
  .md {
    width: var(--icone-md);
    height: var(--icone-md);
  }
  .lg {
    width: var(--icone-lg);
    height: var(--icone-lg);
  }
  path {
    stroke-width: var(--linha-icone);
  }
</style>
