<script>
  // `Badge tipo` (set 478): os 9 tipos. Pílula de 20, fundo selo/neutro/fundo,
  // padding espaco/2 espaco/8 espaco/2 espaco/6, gap espaco/4, ícone 12, Texto/Etiqueta.
  import Icone from './Icone.svelte';
  import { TIPOS } from './tipos.js';

  /** @type {{ tipo: string }} */
  let { tipo } = $props();
  const t = $derived(TIPOS[tipo]);
</script>

<span class="badge"><Icone nome={t.icone} tamanho="xs" />{t.rotulo}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--espaco-4);
    height: calc(var(--altura-linha-xs) + 2 * var(--espaco-2));
    padding: var(--espaco-2) var(--espaco-8) var(--espaco-2) var(--espaco-6);
    border-radius: var(--raio-pilula);
    background: var(--cor-selo-neutro-fundo);
    color: var(--cor-texto-secundario);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    white-space: nowrap;
  }
</style>
