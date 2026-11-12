<script>
  // Text that comes from the agent (command, directory, entry, processes of the chain),
  // already with the visible escape of the Rust side (D51). Each run of RTL letters goes
  // isolated in a `<bdi dir="rtl">` so the text around it is not reordered (Phase 39f,
  // `bidi.js`). Text only, never HTML. No style of its own: inherits the font and the
  // color of whoever uses it.
  import { bidiParts } from '../bidi.js';

  /** @type {{ text: string }} */
  let { text } = $props();
  const parts = $derived(bidiParts(text ?? ''));
</script>

{#each parts as p, i (i)}{#if p.rtl}<bdi dir="rtl">{p.text}</bdi>{:else}{p.text}{/if}{/each}
