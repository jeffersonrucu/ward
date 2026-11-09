<script>
  // `Campo secreto` (set 375): oculto, revelado, copiado. Mesma caixa do `Campo`;
  // ações (gap espaco/4): revelar 32×32, gerar (opcional), `Ação de campo` copiar.
  //
  // S11/D4: este componente nunca busca nem guarda o valor. Oculto, só desenha a
  // máscara. O valor só entra quando a tela dona passa `revelado` (e o tira de novo em
  // 30 s ou ao sair, Fase 33). "Copiar" chama `oncopiar`, que pede ao Rust: o valor não
  // passa por aqui.
  import AcaoDeCampo from './AcaoDeCampo.svelte';
  import Icone from './Icone.svelte';

  /**
   * @type {{
   *   rotulo: string, id: string, revelado?: string | null, copiado?: boolean,
   *   ajuda?: string, vazio?: boolean, mostrarGerar?: boolean,
   *   onrevelar?: () => void, onesconder?: () => void, oncopiar?: () => void, ongerar?: () => void
   * }}
   */
  let {
    rotulo,
    id,
    revelado = null,
    copiado = false,
    ajuda = 'Revelar e copiar não pedem PIN.',
    vazio = false,
    mostrarGerar = false,
    onrevelar = undefined,
    onesconder = undefined,
    oncopiar = undefined,
    ongerar = undefined,
  } = $props();

  const aberto = $derived(revelado !== null);
</script>

<div class="campo">
  <span class="rotulo" id="{id}-rotulo">{rotulo}</span>
  <div class="entrada" role="group" aria-labelledby="{id}-rotulo">
    {#if vazio}
      <span class="valor placeholder">Nenhum valor salvo</span>
    {:else if aberto}
      <span class="valor revelado">{revelado}</span>
    {:else}
      <span class="valor" aria-label="valor oculto">••••••••••••</span>
    {/if}
    <div class="acoes">
      {#if !vazio}
        <button
          class="acao-icone"
          type="button"
          aria-label={aberto ? 'Ocultar' : 'Revelar'}
          aria-pressed={aberto}
          onclick={() => (aberto ? onesconder?.() : onrevelar?.())}
        >
          <Icone nome={aberto ? 'olho-fechado' : 'olho'} />
        </button>
      {/if}
      {#if mostrarGerar}
        <button class="acao-icone" type="button" aria-label="Gerar senha" onclick={() => ongerar?.()}>
          <Icone nome="gerador" />
        </button>
      {/if}
      {#if !vazio}
        <AcaoDeCampo rotulo="Copiar" {copiado} onclick={() => oncopiar?.()} />
      {/if}
    </div>
  </div>
  {#if ajuda}<span class="ajuda">{ajuda}</span>{/if}
</div>

<style>
  .campo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
    min-width: var(--largura-campo);
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .entrada {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    height: var(--controle-padrao);
    padding: var(--espaco-0) var(--espaco-8) var(--espaco-0) var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
  }
  .valor {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .revelado {
    font-family: var(--familia-mono);
    user-select: text;
  }
  .placeholder {
    color: var(--cor-texto-secundario);
  }
  .acoes {
    display: flex;
    align-items: center;
    gap: var(--espaco-4);
  }
  .acao-icone {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-sm);
    height: var(--controle-sm);
    border-radius: var(--raio-sm);
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .acao-icone:hover {
    background: var(--cor-superficie-elevada);
  }
  .ajuda {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
</style>
