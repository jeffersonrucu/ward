<script>
  // Tela `Gerador de senha / Avulso` (FigCraft 13141, Fase 37; V5.21).
  //
  // S11/D4: a senha nasce no Rust (`ward_core::gerador`, pelo comando `gerar_senha`) e
  // fica lá. Esta tela recebe só tamanho, força e o conjunto de símbolos de D35. O valor
  // entra aqui apenas no "revelar" e sai em 30 s, ao esconder, ao gerar de novo ou ao
  // sair da tela. "Copiar" pede ao Rust (`copiar_senha_gerada`), que usa o mesmo
  // caminho de área de transferência do `copiar_campo` (limpeza em 30 s, V5.5). Ao sair
  // da tela, `descartar_senha_gerada` zera a senha no Rust. Nada vai para
  // armazenamento do navegador, URL ou estado global.
  import { onDestroy, onMount } from 'svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import CampoSecreto from '../lib/componentes/CampoSecreto.svelte';
  import ControleDeslizante from '../lib/componentes/ControleDeslizante.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import LinhaDeOpcao from '../lib/componentes/LinhaDeOpcao.svelte';
  import Toast from '../lib/componentes/Toast.svelte';
  import { chamar } from '../lib/ponte/index.js';

  // Espelho de `ward_app_core::gerador::{TAMANHO_MIN, TAMANHO_MAX, TAMANHO_PADRAO}`
  // (o teste `configuracao_tauri` confere). O Rust recusa qualquer outro tamanho.
  const TAMANHO_MIN = 8;
  const TAMANHO_MAX = 64;
  const TAMANHO_PADRAO = 24;
  const PRAZO_REVELAR_MS = 30_000;
  const PRAZO_COPIADO_MS = 2_000;

  const FORCAS = {
    fraca: { rotulo: 'fraca', segmentos: 1, tom: 'perigo' },
    razoavel: { rotulo: 'razoável', segmentos: 2, tom: 'aviso' },
    forte: { rotulo: 'forte', segmentos: 3, tom: 'sucesso' },
    'muito-forte': { rotulo: 'muito forte', segmentos: 4, tom: 'sucesso' },
  };

  let tamanho = $state(TAMANHO_PADRAO);
  let conjuntos = $state({ minusculas: true, maiusculas: true, digitos: true, simbolos: true });
  /** @type {{ tamanho: number, bits: number, forca: string, simbolos: string } | null} */
  let gerada = $state(null);
  /** @type {string | null} */
  let revelado = $state(null);
  let copiado = $state(false);
  /** @type {string | null} */
  let erro = $state(null);

  const forca = $derived(gerada ? FORCAS[gerada.forca] : null);
  const ligados = $derived(Object.values(conjuntos).filter(Boolean).length);

  let prazoRevelar = 0;
  let prazoCopiado = 0;
  let ativa = true;

  function esconder() {
    clearTimeout(prazoRevelar);
    revelado = null;
  }

  // Um pedido por vez: se as opções mudarem durante um pedido, gera de novo no fim, para
  // a senha guardada no Rust ser sempre a das opções da tela.
  let gerando = false;
  let deNovo = false;

  async function gerar() {
    esconder();
    copiado = false;
    if (gerando) {
      deNovo = true;
      return;
    }
    gerando = true;
    try {
      do {
        deNovo = false;
        try {
          const r = await chamar('gerar_senha', { tamanho, ...conjuntos });
          if (ativa) {
            gerada = r;
            erro = null;
          }
        } catch (e) {
          if (ativa) {
            gerada = null;
            erro = e.mensagem;
          }
        }
      } while (deNovo && ativa);
    } finally {
      gerando = false;
    }
  }

  async function revelar() {
    try {
      const r = await chamar('revelar_senha_gerada');
      if (!ativa || gerando) return;
      revelado = r.valor;
      clearTimeout(prazoRevelar);
      prazoRevelar = setTimeout(esconder, PRAZO_REVELAR_MS);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  async function copiar() {
    try {
      await chamar('copiar_senha_gerada');
      copiado = true;
      erro = null;
      clearTimeout(prazoCopiado);
      prazoCopiado = setTimeout(() => (copiado = false), PRAZO_COPIADO_MS);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  function mudarTamanho(v) {
    tamanho = Math.min(TAMANHO_MAX, Math.max(TAMANHO_MIN, Math.round(v)));
    gerar();
  }

  function mudarConjunto(nome, ligado) {
    // O último conjunto ligado não desliga (o Rust também recusa nenhum conjunto).
    if (!ligado && ligados <= 1) return;
    conjuntos[nome] = ligado;
    gerar();
  }

  onMount(gerar);

  onDestroy(() => {
    ativa = false;
    esconder();
    clearTimeout(prazoCopiado);
    chamar('descartar_senha_gerada').catch(() => {});
  });
</script>

<section class="principal" aria-labelledby="gerador-titulo">
  <header class="topo">
    <h1 id="gerador-titulo" class="titulo-pagina">Gerador de senha</h1>
    <p class="resumo">Gera senhas para usar fora do cofre. Nada do que é gerado aqui fica salvo.</p>
  </header>

  <div class="colunas">
    <section class="opcoes" aria-labelledby="opcoes-titulo">
      <h2 id="opcoes-titulo" class="titulo-secao">Opções</h2>
      <div class="tamanho">
        <div class="linha">
          <label class="rotulo" for="gerador-tamanho">Tamanho</label>
          <span class="valor-tamanho">{tamanho} caracteres</span>
        </div>
        <ControleDeslizante
          id="gerador-tamanho"
          rotuloAcessivel="Tamanho da senha"
          valor={tamanho}
          minimo={TAMANHO_MIN}
          maximo={TAMANHO_MAX}
          oninput={mudarTamanho}
        />
        <p class="ajuda">
          De {TAMANHO_MIN} a {TAMANHO_MAX}. Use menos só quando o painel limitar o tamanho.
        </p>
      </div>
      <LinhaDeOpcao
        id="gerador-minusculas"
        titulo="Minúsculas"
        ligado={conjuntos.minusculas}
        desabilitado={conjuntos.minusculas && ligados === 1}
        onmudar={(v) => mudarConjunto('minusculas', v)}
      >
        {#snippet descricao()}a a z{/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao
        id="gerador-maiusculas"
        titulo="Maiúsculas"
        ligado={conjuntos.maiusculas}
        desabilitado={conjuntos.maiusculas && ligados === 1}
        onmudar={(v) => mudarConjunto('maiusculas', v)}
      >
        {#snippet descricao()}A a Z{/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao
        id="gerador-digitos"
        titulo="Dígitos"
        ligado={conjuntos.digitos}
        desabilitado={conjuntos.digitos && ligados === 1}
        onmudar={(v) => mudarConjunto('digitos', v)}
      >
        {#snippet descricao()}0 a 9{/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao
        id="gerador-simbolos"
        titulo="Símbolos"
        ligado={conjuntos.simbolos}
        desabilitado={conjuntos.simbolos && ligados === 1}
        onmudar={(v) => mudarConjunto('simbolos', v)}
      >
        {#snippet descricao()}<span class="simbolos" data-teste="simbolos">{gerada?.simbolos ?? ''}</span>
          Desligue se o painel não aceitar.{/snippet}
      </LinhaDeOpcao>
    </section>

    <section class="resultado" aria-labelledby="resultado-titulo">
      <header class="cabecalho">
        <span class="avatar"><Icone nome="gerador" tamanho="lg" /></span>
        <h2 id="resultado-titulo" class="titulo-modal">Senha gerada</h2>
      </header>

      <div class="corpo">
        <CampoSecreto
          id="gerador-senha"
          rotulo="Senha"
          vazio={!gerada}
          mascara={'•'.repeat(gerada?.tamanho ?? 0)}
          {revelado}
          {copiado}
          onrevelar={revelar}
          onesconder={esconder}
          oncopiar={copiar}
        />

        {#if forca}
          <div class="forca">
            <div class="linha">
              <span class="rotulo">Força</span>
              <span class="valor-forca" data-tom={forca.tom}>
                {forca.rotulo} · cerca de {gerada.bits} bits
              </span>
            </div>
            <div class="barra" aria-hidden="true">
              {#each [1, 2, 3, 4] as n (n)}
                <span class="segmento" data-tom={n <= forca.segmentos ? forca.tom : undefined}
                ></span>
              {/each}
            </div>
          </div>
        {/if}

        <div class="nota">
          <Icone nome="relogio" />
          <div class="textos">
            <span class="titulo-nota">Não fica no cofre</span>
            <span class="texto-nota">
              Para guardar uma senha gerada, abra a credencial e use o gerador no campo Senha.
              Copiar limpa a área de transferência em 30 s.
            </span>
          </div>
        </div>

        {#if erro}<Toast tipo="erro" mensagem={erro} />{/if}
      </div>

      <footer class="rodape">
        <span class="metadados">Gerada agora · não fica no cofre</span>
        <Botao variante="secundario" icone="gerador" onclick={gerar}>Gerar de novo</Botao>
        <!-- O retorno do copiar aparece na `Ação de campo` (variante copiado do design). -->
        <Botao variante="primario" icone="copiar" onclick={copiar} desabilitado={!gerada}>
          Copiar
        </Botao>
      </footer>
    </section>
  </div>
</section>

<style>
  .principal {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    flex: 1;
    min-width: 0;
  }
  .topo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    padding: var(--espaco-4) var(--espaco-8) var(--espaco-0);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .resumo {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .colunas {
    display: flex;
    gap: var(--espaco-16);
    flex: 1;
    min-height: 0;
  }
  .opcoes {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
    flex: none;
    width: var(--largura-coluna-lista);
    padding: var(--espaco-24);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
    overflow-y: auto;
  }
  .tamanho {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
    padding: var(--espaco-20);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .linha {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  .rotulo {
    flex: 1;
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .valor-tamanho {
    color: var(--cor-texto-secundario);
    font-family: var(--familia-mono);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .ajuda {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .simbolos {
    overflow-wrap: anywhere;
    /* A Inter junta `<=>` numa seta: os símbolos de D35 aparecem um a um. */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
  }
  .resultado {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-24);
    border-bottom: var(--linha-fina) solid var(--cor-borda-sutil);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-lg);
    height: var(--controle-lg);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
    flex: 1;
    padding: var(--espaco-24);
    overflow-y: auto;
  }
  .forca {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
  .valor-forca {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .valor-forca[data-tom='perigo'] {
    color: var(--cor-perigo-texto);
  }
  .valor-forca[data-tom='aviso'] {
    color: var(--cor-aviso-texto);
  }
  .valor-forca[data-tom='sucesso'] {
    color: var(--cor-sucesso-texto);
  }
  .barra {
    display: flex;
    gap: var(--espaco-4);
  }
  .segmento {
    flex: 1;
    height: var(--espaco-6);
    border-radius: var(--raio-pilula);
    background: var(--cor-superficie-hover);
  }
  .segmento[data-tom='perigo'] {
    background: var(--cor-perigo-padrao);
  }
  .segmento[data-tom='aviso'] {
    background: var(--cor-aviso-padrao);
  }
  .segmento[data-tom='sucesso'] {
    background: var(--cor-sucesso-padrao);
  }
  .nota {
    display: flex;
    align-items: flex-start;
    gap: var(--espaco-12);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
  }
  .titulo-nota {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .texto-nota {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .rodape {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    padding: var(--espaco-16) var(--espaco-24);
    border-top: var(--linha-fina) solid var(--cor-borda-sutil);
  }
  .metadados {
    flex: 1;
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
</style>
