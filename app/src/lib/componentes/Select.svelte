<script>
  // `Select` (set 421): padrão, foco, aberto, desabilitado. Usa o <select> nativo
  // (teclado e leitor de tela de graça); caixa igual à do `Campo`, largura mínima
  // largura/select-filtro, chevron-baixo à direita.
  import Icone from './Icone.svelte';

  /**
   * @type {{ rotulo: string, id: string, valor?: string,
   *   opcoes: Array<{ valor: string, rotulo: string }>, desabilitado?: boolean }}
   */
  let { rotulo, id, valor = $bindable(''), opcoes, desabilitado = false } = $props();
</script>

<div class="select">
  <label class="rotulo" for={id}>{rotulo}</label>
  <div class="caixa">
    <select {id} bind:value={valor} disabled={desabilitado}>
      {#each opcoes as o (o.valor)}
        <option value={o.valor}>{o.rotulo}</option>
      {/each}
    </select>
    <span class="chevron"><Icone nome="chevron-baixo" tamanho="sm" /></span>
  </div>
</div>

<style>
  .select {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
    min-width: var(--largura-select-filtro);
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .caixa {
    position: relative;
  }
  select {
    appearance: none;
    width: 100%;
    height: var(--controle-padrao);
    padding: var(--espaco-0) var(--espaco-40) var(--espaco-0) var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    cursor: pointer;
  }
  select:focus-visible {
    outline: none;
    border-color: var(--cor-borda-foco);
  }
  select:disabled {
    background: var(--cor-superficie-desabilitada);
    color: var(--cor-texto-desabilitado);
    cursor: not-allowed;
  }
  option {
    background: var(--cor-superficie-padrao);
  }
  .chevron {
    position: absolute;
    right: var(--espaco-16);
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    pointer-events: none;
  }
</style>
