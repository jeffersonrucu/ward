<script>
  // `Caixa de código` (16340): padding espaco/16, gap espaco/4, raio controle, fundo
  // fundo/padrao, borda borda/sutil; rótulo em Texto/Legenda secundário e conteúdo em
  // Mono/Código primário, quebrando em qualquer ponto (o texto do agente pode não ter
  // espaço). O conteúdo entra como texto (nunca HTML).

  /** @type {{ rotulo: string, children: import('svelte').Snippet }} */
  let { rotulo, children } = $props();
</script>

<div class="caixa">
  <span class="rotulo">{rotulo}</span>
  <div class="conteudo">{@render children()}</div>
</div>

<style>
  .caixa {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    width: 100%;
    padding: var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-padrao);
  }
  .rotulo {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .conteudo {
    font-family: var(--familia-mono);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-primario);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    /* D51: sem ligaduras nem troca de forma; cada caractere como é. */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
    /* Sempre da esquerda para a direita, isolado do resto: o primeiro caractere do
       agente não escolhe a direção do bloco. */
    direction: ltr;
    unicode-bidi: isolate;
  }
</style>
