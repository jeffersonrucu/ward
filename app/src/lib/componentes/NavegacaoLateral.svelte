<script>
  // `Navegação lateral` (set 782): Credenciais `783`, Gerador de senha `3037`,
  // Configurações `3077`. Largura largura/barra-lateral, raio painel, fundo/sutil, borda
  // sutil, padding espaco/24 × espaco/16, gap espaco/32. Item: altura controle/md, raio
  // controle, padding espaco/8 × espaco/12, gap espaco/12; ativo com superfície elevada e
  // borda padrão. Rodapé: cartão superfície/padrão com o estado do cofre e "Travar agora".
  import Botao from './Botao.svelte';
  import Icone from './Icone.svelte';
  import Logo from './Logo.svelte';

  /**
   * @type {{
   *   ativo: 'credenciais' | 'gerador' | 'configuracoes',
   *   onnavegar?: (tela: string) => void,
   *   ontravar?: () => void,
   * }}
   */
  let { ativo, onnavegar = undefined, ontravar = undefined } = $props();

  const ITENS = [
    { tela: 'credenciais', rotulo: 'Credenciais', icone: 'chave' },
    { tela: 'gerador', rotulo: 'Gerador de senha', icone: 'gerador' },
    { tela: 'configuracoes', rotulo: 'Configurações', icone: 'engrenagem' },
  ];
</script>

<nav class="lateral" aria-label="Navegação principal">
  <div class="marca"><Logo /></div>
  <ul class="itens">
    {#each ITENS as item (item.tela)}
      <li>
        <button
          type="button"
          class="item"
          class:ativo={item.tela === ativo}
          aria-current={item.tela === ativo ? 'page' : undefined}
          onclick={() => onnavegar?.(item.tela)}
        >
          <Icone nome={item.icone} tom={item.tela === ativo ? 'forte' : undefined} />
          <span>{item.rotulo}</span>
        </button>
      </li>
    {/each}
  </ul>
  <div class="espaco"></div>
  <div class="rodape">
    <div class="estado">
      <span class="indicador" aria-hidden="true"></span>
      <span>Cofre destravado</span>
    </div>
    <div class="travar">
      <Botao variante="secundario" icone="cadeado" onclick={() => ontravar?.()}>Travar agora</Botao>
    </div>
  </div>
</nav>

<style>
  .lateral {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-32);
    flex: none;
    width: var(--largura-barra-lateral);
    padding: var(--espaco-24) var(--espaco-16);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
  }
  .marca {
    display: flex;
    padding: var(--espaco-0) var(--espaco-8);
  }
  .itens {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    width: 100%;
    height: var(--controle-md);
    padding: var(--espaco-8) var(--espaco-12);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid transparent;
    background: transparent;
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    text-align: left;
    cursor: pointer;
  }
  .item:hover:not(.ativo) {
    background: var(--cor-superficie-padrao);
  }
  .item.ativo {
    border-color: var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    color: var(--cor-texto-primario);
    font-weight: var(--peso-medio);
  }
  .espaco {
    flex: 1;
  }
  .rodape {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
    padding: var(--espaco-16);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .estado {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .indicador {
    width: var(--espaco-8);
    height: var(--espaco-8);
    border-radius: var(--raio-pilula);
    background: var(--cor-sucesso-padrao);
  }
  /* Grade de uma coluna: o botão estica na largura do cartão, como no design. */
  .travar {
    display: grid;
  }
</style>
