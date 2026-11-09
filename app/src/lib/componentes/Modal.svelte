<script>
  // `Modal` (set 611): padrão e perigo. Largura largura/dialogo, raio janela, padding
  // espaco/24, gap espaco/20, rodapé gap espaco/12, Sombra/Modal. Usa <dialog> nativo:
  // foco preso no modal e Esc fecha (V5.25).
  import Icone from './Icone.svelte';

  /**
   * @type {{ titulo: string, aberto?: boolean, perigo?: boolean, onfechar?: () => void,
   *   children?: import('svelte').Snippet, rodape?: import('svelte').Snippet }}
   */
  let { titulo, aberto = false, perigo = false, onfechar = undefined, children, rodape } = $props();

  let dialogo = $state();

  $effect(() => {
    if (!dialogo) return;
    if (aberto && !dialogo.open) dialogo.showModal();
    if (!aberto && dialogo.open) dialogo.close();
  });
</script>

<dialog bind:this={dialogo} class="modal" class:perigo aria-labelledby="modal-titulo" onclose={() => onfechar?.()}>
  <div class="cabecalho">
    {#if perigo}<Icone nome="alerta" tom="perigo" />{/if}
    <h2 id="modal-titulo">{titulo}</h2>
  </div>
  {#if children}<div class="texto">{@render children()}</div>{/if}
  {#if rodape}<div class="rodape">{@render rodape()}</div>{/if}
</dialog>

<style>
  .modal {
    width: var(--largura-dialogo);
    max-width: calc(100vw - 2 * var(--espaco-24));
    padding: var(--espaco-24);
    border-radius: var(--raio-janela);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-padrao);
    color: var(--cor-texto-primario);
    box-shadow: var(--sombra-modal);
  }
  .modal[open] {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
  }
  .modal::backdrop {
    background: var(--cor-fundo-sobreposicao);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  h2 {
    margin: 0;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .texto {
    color: var(--cor-texto-secundario);
  }
  .rodape {
    display: flex;
    justify-content: flex-end;
    gap: var(--espaco-12);
  }
</style>
