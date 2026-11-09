<script>
  // `Controle deslizante` (set 12604): trilho de altura espaco/6 em pílula, preenchido
  // primária até a alça e restante superfície/hover; alça icone/md (20) texto/primário com
  // borda linha/grossa borda/foco. Altura total icone/md.
  //
  // É um `<input type="range">` (teclado e leitor). A posição do preenchido vai numa
  // propriedade CSS pelo CSSOM (`style.setProperty`), que a CSP sem 'unsafe-inline'
  // permite; nada de atributo `style` no HTML.

  /**
   * @type {{
   *   id: string, rotuloAcessivel: string, valor: number, minimo: number, maximo: number,
   *   oninput?: (valor: number) => void
   * }}
   */
  let { id, rotuloAcessivel, valor, minimo, maximo, oninput = undefined } = $props();

  /** @type {HTMLInputElement | undefined} */
  let campo = $state();

  $effect(() => {
    const fracao = maximo > minimo ? (valor - minimo) / (maximo - minimo) : 0;
    campo?.style.setProperty('--progresso', String(Math.min(1, Math.max(0, fracao))));
  });
</script>

<input
  bind:this={campo}
  {id}
  class="deslizante"
  type="range"
  min={minimo}
  max={maximo}
  step="1"
  value={valor}
  aria-label={rotuloAcessivel}
  oninput={(e) => oninput?.(Number(e.currentTarget.value))}
/>

<style>
  .deslizante {
    /* Fração preenchida, de 0 a 1 (vem do script). A alça fica sobre o fim do
       preenchido, como no design. */
    --progresso: 0;
    --ponto: calc(var(--icone-md) / 2 + (100% - var(--icone-md)) * var(--progresso));
    appearance: none;
    width: 100%;
    height: var(--icone-md);
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  .deslizante::-webkit-slider-runnable-track {
    height: var(--espaco-6);
    border-radius: var(--raio-pilula);
    background: linear-gradient(
      to right,
      var(--cor-primaria-padrao) 0 var(--ponto),
      var(--cor-superficie-hover) var(--ponto) 100%
    );
  }
  .deslizante::-webkit-slider-thumb {
    appearance: none;
    box-sizing: border-box;
    width: var(--icone-md);
    height: var(--icone-md);
    margin-top: calc((var(--espaco-6) - var(--icone-md)) / 2);
    border-radius: var(--raio-pilula);
    border: var(--linha-grossa) solid var(--cor-borda-foco);
    background: var(--cor-texto-primario);
  }
  .deslizante:focus-visible {
    outline: none;
  }
  .deslizante:focus-visible::-webkit-slider-thumb {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
</style>
