<script>
  // `Anexo` (6849): cartão superfície/padrão, borda sutil, raio item, padding espaco/16,
  // gap espaco/12. Avatar controle/sm (raio controle, superfície elevada, borda padrão)
  // com Ícone/clipe 16; nome em Texto/Corpo forte e detalhe ("1,2 KB ·
  // WARD_ARQUIVO_DEPLOY_PEM") em Texto/Legenda secundária; `Ação de campo` Remover
  // (ícone fechar). O conteúdo do arquivo nunca aparece (S11, V5.12).
  import AcaoDeCampo from './AcaoDeCampo.svelte';
  import Icone from './Icone.svelte';

  /** @type {{ nome: string, detalhe: string, onremover?: () => void }} */
  let { nome, detalhe, onremover = undefined } = $props();
</script>

<div class="anexo" data-anexo={nome}>
  <span class="avatar"><Icone nome="clipe" tamanho="sm" /></span>
  <div class="textos">
    <span class="nome">{nome}</span>
    <span class="detalhe">{detalhe}</span>
  </div>
  {#if onremover}
    <AcaoDeCampo rotulo="Remover" icone="fechar" aria-label="Remover {nome}" onclick={onremover} />
  {/if}
</div>

<style>
  .anexo {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    padding: var(--espaco-16);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .avatar {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--controle-sm);
    height: var(--controle-sm);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
    min-width: 0;
  }
  .nome {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .detalhe {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
</style>
