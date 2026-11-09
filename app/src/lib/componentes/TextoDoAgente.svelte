<script>
  // Texto que vem do agente (comando, diretório, entrada, processos da cadeia), já com
  // o escape visível do Rust (D51). Cada sequência de letras RTL vai isolada num
  // `<bdi dir="rtl">` para não reordenar o texto em volta (Fase 39f, `bidi.js`). Só
  // texto, nunca HTML. Sem estilo próprio: herda a fonte e a cor de quem o usa.
  import { partesBidi } from '../bidi.js';

  /** @type {{ texto: string }} */
  let { texto } = $props();
  const partes = $derived(partesBidi(texto ?? ''));
</script>

{#each partes as p, i (i)}{#if p.rtl}<bdi dir="rtl">{p.texto}</bdi>{:else}{p.texto}{/if}{/each}
