<script>
  // `Botão` (set 263): primário, secundário, perigo, fantasma; altura controle/padrao,
  // raio controle, padding 0 × espaco/20, gap espaco/8, Texto/Corpo forte.
  import Icone from './Icone.svelte';

  /**
   * @type {{
   *   variante?: 'primario' | 'secundario' | 'perigo' | 'fantasma',
   *   tipo?: 'button' | 'submit',
   *   icone?: string,
   *   desabilitado?: boolean,
   *   onclick?: (e: MouseEvent) => void,
   *   children?: import('svelte').Snippet,
   *   [k: string]: unknown
   * }}
   */
  let {
    variante = 'primario',
    tipo = 'button',
    icone = undefined,
    desabilitado = false,
    onclick = undefined,
    children,
    ...resto
  } = $props();

  const tons = { primario: 'sobre-cor', secundario: 'forte', perigo: 'perigo', fantasma: 'forte' };
</script>

<button class="botao {variante}" type={tipo} disabled={desabilitado} {onclick} {...resto}>
  {#if icone}<Icone nome={icone} tom={desabilitado ? 'desabilitado' : tons[variante]} />{/if}
  {#if children}<span>{@render children()}</span>{/if}
</button>

<style>
  .botao {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--espaco-8);
    height: var(--controle-padrao);
    padding: var(--espaco-0) var(--espaco-20);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid transparent;
    font-family: var(--familia-texto);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    white-space: nowrap;
    cursor: pointer;
  }
  .primario {
    background: var(--cor-primaria-padrao);
    color: var(--cor-primaria-sobre);
    box-shadow: var(--sombra-brilho-primario);
  }
  .primario:hover:not(:disabled) {
    background: var(--cor-primaria-hover);
  }
  .secundario {
    background: var(--cor-superficie-elevada);
    border-color: var(--cor-borda-padrao);
    color: var(--cor-texto-primario);
  }
  .secundario:hover:not(:disabled) {
    background: var(--cor-superficie-hover);
  }
  .perigo {
    background: var(--cor-perigo-translucido);
    border-color: var(--cor-perigo-borda);
    color: var(--cor-perigo-texto);
  }
  .perigo:hover:not(:disabled) {
    background: var(--cor-perigo-translucido-hover);
  }
  .fantasma {
    background: transparent;
    color: var(--cor-texto-primario);
  }
  .fantasma:hover:not(:disabled) {
    background: var(--cor-superficie-padrao);
  }
  .botao:disabled {
    background: var(--cor-superficie-desabilitada);
    border-color: transparent;
    color: var(--cor-texto-desabilitado);
    box-shadow: none;
    cursor: not-allowed;
  }
</style>
