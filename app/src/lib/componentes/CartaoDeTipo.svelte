<script>
  // `Cartão de tipo` (17607): padding espaco/20 espaco/16, gap espaco/12, raio cartão;
  // avatar controle/md com o ícone do tipo; nome Texto/Corpo forte; descrição
  // Texto/Pequeno secundária. Hover: superfície elevada e borda padrão; selecionado:
  // superfície selecionada e borda foco.
  import Icone from './Icone.svelte';
  import { tipoDe } from './tipos.js';

  /**
   * @type {{ tipo: string, nome: string, descricao: string, selecionado?: boolean,
   *   desabilitado?: boolean, onclick?: () => void }}
   */
  let {
    tipo,
    nome,
    descricao,
    selecionado = false,
    desabilitado = false,
    onclick = undefined,
  } = $props();
</script>

<button
  type="button"
  class="cartao"
  class:selecionado
  role="radio"
  aria-checked={selecionado}
  disabled={desabilitado}
  data-tipo={tipo}
  onclick={() => onclick?.()}
>
  <span class="avatar">
    <Icone nome={tipoDe(tipo).icone} tom={selecionado ? 'marca' : undefined} />
  </span>
  <span class="nome">{nome}</span>
  <span class="descricao">{descricao}</span>
</button>

<style>
  .cartao {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--espaco-12);
    padding: var(--espaco-20) var(--espaco-16);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
    color: var(--cor-texto-primario);
    text-align: left;
    cursor: pointer;
  }
  .cartao:hover:not(:disabled) {
    background: var(--cor-superficie-elevada);
    border-color: var(--cor-borda-padrao);
  }
  .cartao.selecionado {
    background: var(--cor-superficie-selecionada);
    border-color: var(--cor-borda-foco);
  }
  .cartao:disabled {
    cursor: not-allowed;
    color: var(--cor-texto-desabilitado);
  }
  .cartao:focus-visible {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-md);
    height: var(--controle-md);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .descricao {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .cartao:disabled .descricao {
    color: var(--cor-texto-desabilitado);
  }
</style>
