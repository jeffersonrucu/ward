<script>
  // `Campo PIN` (set 650): vazio, digitando, completo, erro, bloqueado. Dígitos 48×48
  // (controle/lg), raio controle, gap espaco/12, máscara Mono/PIN. Dígito cheio com
  // borda borda/padrao e erro com perigo/texto (1055 e 1106, conferidos na Fase 35).
  //
  // O PIN é um único <input> (inputmode numérico, sem autocompletar); as caixas são só
  // desenho. O valor fica no estado local do componente e a tela dona o limpa depois de
  // enviar. Aceita só dígitos e no máximo `digitos` (4 a 6).

  /**
   * @type {{ rotulo: string, id: string, valor?: string, digitos?: 4 | 5 | 6,
   *   ajuda?: string, erro?: string, bloqueado?: boolean, onenviar?: (pin: string) => void }}
   */
  let {
    rotulo,
    id,
    valor = $bindable(''),
    digitos = 6,
    ajuda = '',
    erro = '',
    bloqueado = false,
    onenviar = undefined,
  } = $props();

  function aoDigitar(e) {
    const limpo = e.currentTarget.value.replace(/\D/g, '').slice(0, digitos);
    e.currentTarget.value = limpo;
    valor = limpo;
  }

  function aoTeclar(e) {
    if (e.key === 'Enter' && valor.length >= 4) onenviar?.(valor);
  }
</script>

<div class="pin" class:com-erro={erro} class:bloqueado>
  <label class="rotulo" for={id}>{rotulo}</label>
  <div class="caixas">
    <input
      {id}
      class="entrada"
      type="password"
      inputmode="numeric"
      autocomplete="off"
      maxlength={digitos}
      value={valor}
      disabled={bloqueado}
      aria-invalid={erro ? 'true' : undefined}
      oninput={aoDigitar}
      onkeydown={aoTeclar}
    />
    {#each Array.from({ length: digitos }, (_, i) => i) as i (i)}
      <span class="digito" class:cheio={i < valor.length} class:atual={i === valor.length} aria-hidden="true">
        {i < valor.length ? '•' : ''}
      </span>
    {/each}
  </div>
  {#if bloqueado}
    <span class="ajuda erro">PIN bloqueado: o cofre foi travado.</span>
  {:else if erro}
    <span class="ajuda erro">{erro}</span>
  {:else if ajuda}
    <span class="ajuda">{ajuda}</span>
  {/if}
</div>

<style>
  .pin {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .caixas {
    position: relative;
    display: flex;
    gap: var(--espaco-12);
  }
  .entrada {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: text;
  }
  .digito {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-lg);
    height: var(--controle-lg);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
    font-family: var(--familia-mono);
    font-weight: var(--peso-negrito);
    font-size: var(--tamanho-xl);
    line-height: var(--altura-linha-xl);
  }
  .digito.cheio {
    border-color: var(--cor-borda-padrao);
  }
  .caixas:focus-within .atual {
    border-color: var(--cor-borda-foco);
  }
  .com-erro .digito {
    border-color: var(--cor-perigo-texto);
  }
  .bloqueado .digito {
    background: var(--cor-superficie-desabilitada);
    color: var(--cor-texto-desabilitado);
  }
  .ajuda {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .erro {
    color: var(--cor-perigo-texto);
  }
</style>
