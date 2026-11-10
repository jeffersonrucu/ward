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
  import Button from '../lib/components/Button.svelte';
  import SecretField from '../lib/components/SecretField.svelte';
  import Slider from '../lib/components/Slider.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import OptionRow from '../lib/components/OptionRow.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';

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
          const r = await call('gerar_senha', { tamanho, ...conjuntos });
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
      const r = await call('revelar_senha_gerada');
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
      await call('copiar_senha_gerada');
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
    call('descartar_senha_gerada').catch(() => {});
  });
</script>

<section class="principal" aria-labelledby="gerador-titulo">
  <header class="topo">
    <h1 id="gerador-titulo" class="title-page">Gerador de senha</h1>
    <p class="resumo">Gera senhas para usar fora do cofre. Nada do que é gerado aqui fica salvo.</p>
  </header>

  <div class="colunas">
    <section class="opcoes" aria-labelledby="opcoes-titulo">
      <h2 id="opcoes-titulo" class="title-section">Opções</h2>
      <div class="tamanho">
        <div class="linha">
          <label class="rotulo" for="gerador-tamanho">Tamanho</label>
          <span class="valor-tamanho">{tamanho} caracteres</span>
        </div>
        <Slider
          id="gerador-tamanho"
          accessibleLabel="Tamanho da senha"
          value={tamanho}
          min={TAMANHO_MIN}
          max={TAMANHO_MAX}
          oninput={mudarTamanho}
        />
        <p class="ajuda">
          De {TAMANHO_MIN} a {TAMANHO_MAX}. Use menos só quando o painel limitar o tamanho.
        </p>
      </div>
      <OptionRow
        id="gerador-minusculas"
        title="Minúsculas"
        on={conjuntos.minusculas}
        disabled={conjuntos.minusculas && ligados === 1}
        onchange={(v) => mudarConjunto('minusculas', v)}
      >
        {#snippet description()}a a z{/snippet}
      </OptionRow>
      <OptionRow
        id="gerador-maiusculas"
        title="Maiúsculas"
        on={conjuntos.maiusculas}
        disabled={conjuntos.maiusculas && ligados === 1}
        onchange={(v) => mudarConjunto('maiusculas', v)}
      >
        {#snippet description()}A a Z{/snippet}
      </OptionRow>
      <OptionRow
        id="gerador-digitos"
        title="Dígitos"
        on={conjuntos.digitos}
        disabled={conjuntos.digitos && ligados === 1}
        onchange={(v) => mudarConjunto('digitos', v)}
      >
        {#snippet description()}0 a 9{/snippet}
      </OptionRow>
      <OptionRow
        id="gerador-simbolos"
        title="Símbolos"
        on={conjuntos.simbolos}
        disabled={conjuntos.simbolos && ligados === 1}
        onchange={(v) => mudarConjunto('simbolos', v)}
      >
        {#snippet description()}<span class="simbolos" data-teste="simbolos">{gerada?.simbolos ?? ''}</span>
          Desligue se o painel não aceitar.{/snippet}
      </OptionRow>
    </section>

    <section class="resultado" aria-labelledby="resultado-titulo">
      <header class="cabecalho">
        <span class="avatar"><Icon name="generator" size="lg" /></span>
        <h2 id="resultado-titulo" class="title-modal">Senha gerada</h2>
      </header>

      <div class="corpo">
        <SecretField
          id="gerador-senha"
          label="Senha"
          empty={!gerada}
          mask={'•'.repeat(gerada?.tamanho ?? 0)}
          revealed={revelado}
          copied={copiado}
          onreveal={revelar}
          onhide={esconder}
          oncopy={copiar}
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
          <Icon name="clock" />
          <div class="textos">
            <span class="titulo-nota">Não fica no cofre</span>
            <span class="texto-nota">
              Para guardar uma senha gerada, abra a credencial e use o gerador no campo Senha.
              Copiar limpa a área de transferência em 30 s.
            </span>
          </div>
        </div>

        {#if erro}<Toast kind="error" message={erro} />{/if}
      </div>

      <footer class="rodape">
        <span class="metadados">Gerada agora · não fica no cofre</span>
        <Button variant="secondary" icon="generator" onclick={gerar}>Gerar de novo</Button>
        <!-- O retorno do copiar aparece na `Ação de campo` (variante copiado do design). -->
        <Button variant="primary" icon="copy" onclick={copiar} disabled={!gerada}>
          Copiar
        </Button>
      </footer>
    </section>
  </div>
</section>

<style>
  .principal {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    flex: 1;
    min-width: 0;
  }
  .topo {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-8) var(--space-0);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .resumo {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .colunas {
    display: flex;
    gap: var(--space-16);
    flex: 1;
    min-height: 0;
  }
  .opcoes {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    flex: none;
    width: var(--width-column-list);
    padding: var(--space-24);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow-y: auto;
  }
  .tamanho {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding: var(--space-20);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .linha {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  .rotulo {
    flex: 1;
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .valor-tamanho {
    color: var(--color-text-secondary);
    font-family: var(--font-family-mono);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .ajuda {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
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
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .forca {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .valor-forca {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .valor-forca[data-tom='perigo'] {
    color: var(--color-danger-text);
  }
  .valor-forca[data-tom='aviso'] {
    color: var(--color-warning-text);
  }
  .valor-forca[data-tom='sucesso'] {
    color: var(--color-success-text);
  }
  .barra {
    display: flex;
    gap: var(--space-4);
  }
  .segmento {
    flex: 1;
    height: var(--space-6);
    border-radius: var(--radius-pill);
    background: var(--color-surface-hover);
  }
  .segmento[data-tom='perigo'] {
    background: var(--color-danger-default);
  }
  .segmento[data-tom='aviso'] {
    background: var(--color-warning-default);
  }
  .segmento[data-tom='sucesso'] {
    background: var(--color-success-default);
  }
  .nota {
    display: flex;
    align-items: flex-start;
    gap: var(--space-12);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
  }
  .titulo-nota {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .texto-nota {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .rodape {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16) var(--space-24);
    border-top: var(--line-thin) solid var(--color-border-subtle);
  }
  .metadados {
    flex: 1;
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
</style>
