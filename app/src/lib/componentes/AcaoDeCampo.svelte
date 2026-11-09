<script>
  // `Ação de campo` (set 2882): padrão e copiado; altura controle/sm, raio sm,
  // padding 0 espaco/12 0 espaco/8, gap espaco/6, ícone 16, Texto/Rótulo.
  import Icone from './Icone.svelte';

  /** @type {{ rotulo: string, icone?: string, copiado?: boolean, onclick?: () => void, desabilitado?: boolean, [k: string]: unknown }} */
  let {
    rotulo,
    icone = 'copiar',
    copiado = false,
    onclick = undefined,
    desabilitado = false,
    ...resto
  } = $props();
</script>

<button class="acao" class:copiado type="button" disabled={desabilitado} {onclick} {...resto}>
  <Icone nome={copiado ? 'check' : icone} tamanho="sm" tom={copiado ? 'sucesso' : undefined} />
  <span>{copiado ? 'Copiado' : rotulo}</span>
</button>

<style>
  .acao {
    display: inline-flex;
    align-items: center;
    gap: var(--espaco-6);
    height: var(--controle-sm);
    padding: var(--espaco-0) var(--espaco-12) var(--espaco-0) var(--espaco-8);
    border-radius: var(--raio-sm);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    color: var(--cor-texto-primario);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    cursor: pointer;
    white-space: nowrap;
  }
  .acao:hover:not(:disabled) {
    background: var(--cor-superficie-hover);
  }
  .copiado {
    background: var(--cor-sucesso-sutil);
    border-color: var(--cor-sucesso-texto);
    color: var(--cor-sucesso-texto);
  }
  .acao:disabled {
    color: var(--cor-texto-desabilitado);
    cursor: not-allowed;
  }
</style>
