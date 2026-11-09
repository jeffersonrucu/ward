<script>
  // `Diálogo de filtros` (5222): largura largura/dialogo, raio janela, padding
  // espaco/24, gap espaco/20, fundo superficie/elevada, borda borda/padrao, Sombra/Modal.
  // `cabeçalho` (gap espaco/12): título em Título/Modal e fechar; `Select` Grupo, Tipo e
  // Ambiente; `rodapé` (padding-top espaco/4, gap espaco/12): "Limpar filtros"
  // (fantasma), espaço, "Cancelar" (secundário) e "Aplicar" (primário).
  //
  // <dialog> nativo: foco preso no diálogo e Esc fecha (como "Cancelar"). Os valores
  // escolhidos só valem ao aplicar.
  import Botao from './Botao.svelte';
  import Icone from './Icone.svelte';
  import Select from './Select.svelte';

  /**
   * @typedef {{ grupo: string, tipo: string, ambiente: string }} Filtros
   * @type {{
   *   aberto?: boolean, filtros: Filtros,
   *   grupos: Array<{ valor: string, rotulo: string }>,
   *   tipos: Array<{ valor: string, rotulo: string }>,
   *   ambientes: Array<{ valor: string, rotulo: string }>,
   *   onaplicar?: (f: Filtros) => void, onfechar?: () => void
   * }}
   */
  let {
    aberto = false,
    filtros,
    grupos,
    tipos,
    ambientes,
    onaplicar = undefined,
    onfechar = undefined,
  } = $props();

  let dialogo = $state();
  let grupo = $state('');
  let tipo = $state('');
  let ambiente = $state('');

  $effect(() => {
    if (!dialogo) return;
    if (aberto && !dialogo.open) {
      grupo = filtros.grupo;
      tipo = filtros.tipo;
      ambiente = filtros.ambiente;
      dialogo.showModal();
    }
    if (!aberto && dialogo.open) dialogo.close();
  });

  function limpar() {
    grupo = '';
    tipo = '';
    ambiente = '';
  }

  function aplicar() {
    onaplicar?.({ grupo, tipo, ambiente });
  }
</script>

<dialog
  bind:this={dialogo}
  class="dialogo"
  aria-labelledby="filtros-titulo"
  onclose={() => onfechar?.()}
>
  <div class="cabecalho">
    <h2 id="filtros-titulo">Filtrar credenciais</h2>
    <button class="fechar" type="button" aria-label="Fechar" onclick={() => onfechar?.()}>
      <Icone nome="fechar" />
    </button>
  </div>
  <Select rotulo="Grupo" id="filtro-grupo" bind:valor={grupo} opcoes={grupos} />
  <Select rotulo="Tipo" id="filtro-tipo" bind:valor={tipo} opcoes={tipos} />
  <Select rotulo="Ambiente" id="filtro-ambiente" bind:valor={ambiente} opcoes={ambientes} />
  <div class="rodape">
    <Botao variante="fantasma" onclick={limpar}>Limpar filtros</Botao>
    <span class="espaco"></span>
    <Botao variante="secundario" onclick={() => onfechar?.()}>Cancelar</Botao>
    <Botao variante="primario" onclick={aplicar}>Aplicar</Botao>
  </div>
</dialog>

<style>
  .dialogo {
    width: var(--largura-dialogo);
    max-width: calc(100vw - 2 * var(--espaco-24));
    padding: var(--espaco-24);
    border-radius: var(--raio-janela);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    color: var(--cor-texto-primario);
    box-shadow: var(--sombra-modal);
  }
  .dialogo[open] {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
  }
  .dialogo::backdrop {
    background: var(--cor-fundo-sobreposicao);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  h2 {
    flex: 1;
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .fechar {
    display: inline-flex;
    padding: var(--espaco-0);
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    cursor: pointer;
  }
  .rodape {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    padding-top: var(--espaco-4);
  }
  .espaco {
    flex: 1;
  }
</style>
