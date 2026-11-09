<script>
  // `Contagem` (set 16348): pílula de 20 (padding espaco/2 espaco/8 espaco/2 espaco/6,
  // gap espaco/4, raio pílula), ícone relógio 12 e o tempo em Texto/Etiqueta.
  // `Estado=normal` (16335): fundo aviso/sutil, texto e ícone aviso. `Estado=urgente`
  // (16343): fundo perigo/sutil, texto e ícone perigo.
  import Icone from './Icone.svelte';

  /** @type {{ segundos: number, urgente?: boolean }} */
  let { segundos, urgente = false } = $props();

  const texto = $derived.by(() => {
    const s = Math.max(0, Math.floor(segundos));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')} restantes`;
  });
</script>

<span class="contagem" class:urgente role="timer" aria-live="off">
  <Icone nome="relogio" tamanho="xs" tom={urgente ? 'perigo' : 'aviso'} />
  {texto}
</span>

<style>
  .contagem {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--espaco-4);
    height: calc(var(--altura-linha-xs) + 2 * var(--espaco-2));
    padding: var(--espaco-2) var(--espaco-8) var(--espaco-2) var(--espaco-6);
    border-radius: var(--raio-pilula);
    background: var(--cor-aviso-sutil);
    color: var(--cor-aviso-texto);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .urgente {
    background: var(--cor-perigo-sutil);
    color: var(--cor-perigo-texto);
  }
</style>
