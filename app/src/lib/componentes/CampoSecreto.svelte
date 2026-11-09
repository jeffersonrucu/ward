<script>
  // `Campo secreto` (set 375): oculto, revelado, copiado e vazio (18609). Mesma caixa do
  // `Campo`; ações (gap espaco/4): revelar 32×32, gerar (opcional, `Mostrar gerar`),
  // `Ação de campo` copiar (`Mostrar copiar`). Vazio: placeholder em texto/secundário,
  // sem revelar nem copiar, ajuda fixa.
  //
  // S11/D4: este componente nunca busca nem guarda o valor do cofre. Oculto, só desenha
  // a máscara. O valor só entra quando a tela dona passa `revelado` (e o tira de novo em
  // 30 s, ao esconder ou ao sair). "Copiar" chama `oncopiar`, que pede ao Rust: o valor
  // não passa por aqui.
  //
  // `editavel` (formulário, Fase 33): no estado vazio a caixa é um campo de senha onde o
  // usuário digita (`bind:digitado`); o olho alterna a digitação visível só no próprio
  // campo. Com valor salvo, "Trocar valor" (`ontrocar`) passa a caixa a digitação.
  import AcaoDeCampo from './AcaoDeCampo.svelte';
  import Icone from './Icone.svelte';

  /**
   * @type {{
   *   rotulo: string, id: string, revelado?: string | null, copiado?: boolean,
   *   ajuda?: string, vazio?: boolean, mostrarGerar?: boolean, mascara?: string,
   *   mostrarRevelar?: boolean, mostrarCopiar?: boolean, editavel?: boolean,
   *   digitado?: string, placeholder?: string, erro?: string, multilinha?: boolean,
   *   onrevelar?: () => void, onesconder?: () => void, oncopiar?: () => void,
   *   ongerar?: () => void, ontrocar?: () => void
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
    // A máscara não sai do valor: quem conhece o tamanho (o gerador) pode passar um
    // ponto por caractere; o padrão tem 12.
    mascara = '••••••••••••',
    mostrarRevelar = true,
    mostrarCopiar = true,
    editavel = false,
    digitado = $bindable(''),
    placeholder = 'Nenhum valor salvo',
    erro = '',
    onrevelar = undefined,
    onesconder = undefined,
    oncopiar = undefined,
    ongerar = undefined,
    ontrocar = undefined,
  } = $props();

  const aberto = $derived(revelado !== null);
  // Digitação visível só no campo (o valor já está na caixa, digitado pelo usuário).
  let visivel = $state(false);
  const descricao = $derived(erro || ajuda ? `${id}-ajuda` : undefined);
</script>

<div class="campo" class:com-erro={erro}>
  {#if editavel && vazio}
    <label class="rotulo" for={id}>{rotulo}</label>
  {:else}
    <span class="rotulo" id="{id}-rotulo">{rotulo}</span>
  {/if}
  <div
    class="entrada"
    role={editavel && vazio ? undefined : 'group'}
    aria-labelledby={editavel && vazio ? undefined : `${id}-rotulo`}
  >
    {#if editavel && vazio}
      <input
        {id}
        class="valor digitavel"
        class:mono={visivel && digitado}
        type={visivel ? 'text' : 'password'}
        bind:value={digitado}
        {placeholder}
        autocomplete="new-password"
        spellcheck="false"
        aria-invalid={erro ? 'true' : undefined}
        aria-describedby={descricao}
      />
    {:else if vazio}
      <span class="valor placeholder">{placeholder}</span>
    {:else if aberto}
      <span class="valor revelado">{revelado}</span>
    {:else if ontrocar}
      <button
        type="button"
        class="valor mascara-botao"
        aria-label="Trocar o valor de {rotulo} (valor oculto)"
        title="Trocar valor"
        onclick={() => ontrocar?.()}>{mascara}</button
      >
    {:else}
      <span class="valor" aria-label="valor oculto">{mascara}</span>
    {/if}
    <div class="acoes">
      {#if editavel && vazio && digitado}
        <button
          class="acao-icone"
          type="button"
          aria-label={visivel ? 'Ocultar o que foi digitado' : 'Mostrar o que foi digitado'}
          aria-pressed={visivel}
          onclick={() => (visivel = !visivel)}
        >
          <Icone nome={visivel ? 'olho-fechado' : 'olho'} />
        </button>
      {/if}
      {#if !vazio && mostrarRevelar}
        <button
          class="acao-icone"
          type="button"
          aria-label={aberto ? `Ocultar ${rotulo}` : `Revelar ${rotulo}`}
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
      {#if !vazio && mostrarCopiar}
        <AcaoDeCampo
          rotulo="Copiar"
          {copiado}
          onclick={() => oncopiar?.()}
          aria-label="Copiar {rotulo}"
        />
      {/if}
    </div>
  </div>
  {#if erro}
    <span class="ajuda erro" id="{id}-ajuda">{erro}</span>
  {:else if ajuda}
    <span class="ajuda" id="{id}-ajuda">{ajuda}</span>
  {/if}
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
  .entrada:focus-within {
    border-color: var(--cor-borda-foco);
  }
  .com-erro .entrada {
    border-color: var(--cor-perigo-borda);
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
  .digitavel {
    height: 100%;
    padding: var(--espaco-0);
    border: none;
    background: transparent;
    color: var(--cor-texto-primario);
  }
  .digitavel:focus-visible {
    outline: none;
  }
  .digitavel::placeholder {
    color: var(--cor-texto-secundario);
  }
  .mascara-botao {
    padding: var(--espaco-0);
    border: none;
    background: transparent;
    color: var(--cor-texto-primario);
    text-align: left;
    cursor: text;
  }
  .mascara-botao:focus-visible {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
  .revelado,
  .mono {
    font-family: var(--familia-mono);
    user-select: text;
    /* O valor aparece caractere a caractere: sem ligaduras (`<=` ou `->` virariam um
       glifo só e a senha lida não seria a copiada). */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
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
  .erro {
    color: var(--cor-perigo-texto);
  }
</style>
