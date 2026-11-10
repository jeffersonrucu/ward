<script>
  // `Kit de emergência / Impressão` (FigCraft 16773, Fase 49; V7.10, D17).
  //
  // Aberto por `Configurações › Kit de emergência › Imprimir`. Mostra a folha A4
  // (794×1123, `pagina/a4-*`, papel branco e tinta escura `impressao/*`) numa janela
  // por cima do app, com "Fechar" e "Imprimir". "Imprimir" chama `window.print()`: a
  // janela de impressão do sistema decide a impressora, e um PDF só existe se o usuário
  // escolher "Imprimir em arquivo" e o lugar. O app não grava nada em disco.
  //
  // A folha não tem segredo: as pastas são as fixas da instalação e do daemon vem só o
  // backup externo (`backup_externo`: ligado, UUID e rótulo do pendrive, se conectado;
  // tudo passa por `linhaDoPendrive`). A senha mestra é escrita à mão nas
  // duas linhas em branco; não existe campo, valor nem estado para ela.
  //
  // Na impressão (`@media print`), só a folha sai: o resto da página some. Para isso o
  // <dialog> vai para o fim do <body> (fora do `#app`) enquanto está aberto.
  import Button from '../lib/components/Button.svelte';
  import Logo from '../lib/components/Logo.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';
  import {
    PASSOS_RESTAURAR,
    PASTA_BACKUPS,
    PASTA_COFRE,
    dataImpressa,
    linhaDoPendrive,
  } from './kit-emergencia.js';
  import { mensagemDoRust } from './primeiro-uso.js';

  /** @type {{ aberto?: boolean, onfechar?: () => void, ontravado?: () => void }} */
  let { aberto = false, onfechar = undefined, ontravado = undefined } = $props();

  let dialogo = $state();
  let externo = $state(null);
  let pronto = $state(false);
  let erro = $state('');
  let data = $state(dataImpressa(new Date()));
  let leitura = 0;

  const pendrive = $derived(linhaDoPendrive(externo));

  /** Leva o <dialog> para o fim do <body>, para a impressão esconder todo o resto. */
  function noBody(no) {
    document.body.appendChild(no);
    return { destroy: () => no.remove() };
  }

  async function carregar() {
    const minha = ++leitura;
    pronto = false;
    erro = '';
    externo = null;
    data = dataImpressa(new Date());
    try {
      // Fase 48: `backup_externo` traz ligado, UUID (D45) e o pendrive, se conectado
      // (rótulo já neutralizado no Rust). Nenhum caminho de montagem, nenhum segredo.
      const b = await call('backup_externo');
      if (minha !== leitura) return;
      externo = {
        ligado: b?.ligado === true,
        uuid: typeof b?.uuid === 'string' ? b.uuid : null,
        pendrive: b?.pendrive && typeof b.pendrive.rotulo === 'string' ? { rotulo: b.pendrive.rotulo } : null,
      };
      pronto = true;
    } catch (e) {
      if (minha !== leitura) return;
      if (e?.codigo === 'travado') {
        fechar();
        ontravado?.();
        return;
      }
      erro = mensagemDoRust(e?.mensagem) || 'Não foi possível ler o backup externo. Tente de novo.';
    }
  }

  function fechar() {
    leitura++;
    if (dialogo?.open) dialogo.close();
    else onfechar?.();
  }

  function imprimir() {
    if (!pronto) return;
    window.print();
  }

  $effect(() => {
    if (!dialogo) return;
    if (aberto && !dialogo.open) {
      dialogo.showModal();
      carregar();
    }
    if (!aberto && dialogo.open) dialogo.close();
  });
</script>

<dialog
  bind:this={dialogo}
  use:noBody
  class="kit"
  aria-labelledby="kit-titulo"
  onclose={() => {
    leitura++;
    onfechar?.();
  }}
>
  <header class="barra">
    <div class="textos">
      <h2 id="kit-titulo">Kit de emergência</h2>
      <p>
        Confira a folha e imprima. Para guardar em PDF, escolha imprimir em arquivo na janela de
        impressão; o Ward não grava a folha.
      </p>
    </div>
    <div class="acoes">
      <Button variant="secondary" onclick={fechar}>Fechar</Button>
      <Button onclick={imprimir} disabled={!pronto} aria-label="Imprimir a folha">Imprimir</Button>
    </div>
  </header>
  {#if erro}
    <div class="aviso">
      <Toast kind="error" message={erro} />
      <Button variant="secondary" onclick={carregar}>Tentar de novo</Button>
    </div>
  {/if}

  <div class="rolagem">
    <article class="folha" aria-label="Folha do kit de emergência" data-teste="folha-kit">
      <header class="cabecalho">
        <Logo print />
        <h1 class="titulo">Kit de emergência</h1>
        <p class="texto">
          Guarde esta folha num lugar seguro, longe do computador. Com ela você encontra o cofre e
          os backups mesmo se esta máquina parar.
        </p>
        <p class="data">{data}</p>
      </header>

      <section class="secao">
        <h2 class="title-section">Onde está o cofre</h2>
        <div class="item">
          <span class="rotulo">Pasta do cofre nesta máquina</span>
          <span class="valor mono">{PASTA_COFRE}</span>
        </div>
      </section>

      <section class="secao">
        <h2 class="title-section">Onde estão os backups</h2>
        <div class="item">
          <span class="rotulo">Cópias locais · 30 versões, uma a cada mudança</span>
          <span class="valor mono">{PASTA_BACKUPS}</span>
        </div>
        <div class="item" data-teste="kit-pendrive">
          <span class="rotulo">{pendrive.rotulo}</span>
          {#if pronto}
            <span class="valor" class:mono={pendrive.mono}><bdi>{pendrive.valor}</bdi></span>
          {:else}
            <span class="valor">Lendo o backup externo…</span>
          {/if}
        </div>
      </section>

      <section class="secao">
        <h2 class="title-section">Senha mestra</h2>
        <p class="texto">Escreva à mão. O Ward nunca preenche este espaço.</p>
        <div class="linhas" aria-hidden="true">
          <div class="linha"></div>
          <div class="linha"></div>
        </div>
      </section>

      <section class="secao">
        <h2 class="title-section">Como restaurar</h2>
        <ol class="passos">
          {#each PASSOS_RESTAURAR as passo, i (i)}
            <li>{passo}</li>
          {/each}
        </ol>
      </section>

      <div class="espaco"></div>
      <p class="rodape">
        O PIN de aprovação não aparece nesta folha: ele não abre o cofre. Depois de anotar a senha
        mestra, guarde a folha como guardaria a própria senha.
      </p>
    </article>
  </div>
</dialog>

<style>
  /* Janela por cima do app (tema escuro); a folha é papel branco. */
  .kit {
    width: 100%;
    height: 100%;
    max-width: none;
    max-height: none;
    margin: 0;
    padding: 0;
    border: none;
    background: var(--color-bg-default);
    color: var(--color-text-primary);
  }
  .kit[open] {
    display: flex;
    flex-direction: column;
  }
  .kit::backdrop {
    background: var(--color-bg-overlay);
  }
  .barra {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-24);
    padding: var(--space-16) var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .barra .textos {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .barra h2 {
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .barra p {
    margin: 0;
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .acoes {
    display: flex;
    gap: var(--space-12);
    flex: none;
  }
  .aviso {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-12) var(--space-24) var(--space-0);
  }
  .rolagem {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-24);
  }

  /* Folha A4 (16773): 794×1123, padding espaco/64, gap espaco/32, papel impressao/papel. */
  .folha {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    width: var(--page-a4-width);
    height: var(--page-a4-height);
    margin: 0 auto;
    padding: var(--space-64);
    overflow: hidden;
    background: var(--color-print-paper);
    color: var(--color-print-text);
    box-shadow: var(--shadow-modal);
    font-family: var(--font-family-text);
    print-color-adjust: exact;
    -webkit-print-color-adjust: exact;
  }
  .folha h1,
  .folha h2,
  .folha p,
  .folha ol {
    margin: 0;
  }
  /* cabeçalho (16774): gap espaco/16, padding-bottom espaco/24, borda inferior linha/fina. */
  .cabecalho {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-16);
    padding-bottom: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-print-line);
  }
  /* Título/Seção (8:9). */
  .titulo {
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  /* Texto/Corpo (8:12) em impressao/texto-secundario. */
  .texto {
    color: var(--color-print-text-secondary);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  /* Texto/Legenda (8:16). */
  .data,
  .rodape {
    color: var(--color-print-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  /* seção (16775–16778): gap espaco/12. */
  .secao {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  /* Título/Subseção (8:11). */
  .title-section {
    font-weight: var(--weight-semibold);
    font-size: var(--size-md);
    line-height: var(--line-height-md);
  }
  /* item (16790): gap espaco/4; rótulo Texto/Rótulo (8:15), valor Mono/Corpo (8:18). */
  .item {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .rotulo {
    color: var(--color-print-text-secondary);
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .valor {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    overflow-wrap: anywhere;
  }
  .valor bdi {
    unicode-bidi: isolate;
  }
  .mono {
    font-family: var(--font-family-mono);
  }
  /* linhas (16796): padding espaco/32 em cima e espaco/8 embaixo, gap espaco/40; cada
     linha é uma borda linha/fina em impressao/linha (borda sai na impressão sem fundo). */
  .linhas {
    display: flex;
    flex-direction: column;
    gap: var(--space-40);
    padding: var(--space-32) var(--space-0) var(--space-8);
  }
  .linha {
    border-top: var(--line-thin) solid var(--color-print-line);
  }
  /* passos (16798): Texto/Corpo em impressao/texto, numerados "1." … "4.". */
  .passos {
    padding: var(--space-0);
    list-style-position: inside;
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .espaco {
    flex: 1;
  }

  /* Impressão: só a folha, numa página A4 sem margem (a folha já tem espaco/64). */
  @media print {
    @page {
      size: A4;
      margin: 0;
    }
    /* Só com o kit aberto: um Ctrl+P em outra tela não perde a página. */
    :global(html:has(> body > dialog.kit[open])),
    :global(body:has(> dialog.kit[open])) {
      height: auto;
      background: var(--color-print-paper);
    }
    :global(body:has(> dialog.kit[open]) > :not(dialog.kit)) {
      display: none !important;
    }
    .kit {
      position: static;
      width: auto;
      height: auto;
      overflow: visible;
      background: var(--color-print-paper);
    }
    .kit::backdrop {
      background: transparent;
    }
    .barra,
    .aviso {
      display: none;
    }
    .rolagem {
      overflow: visible;
      padding: 0;
    }
    .folha {
      margin: 0;
      box-shadow: none;
      break-inside: avoid;
    }
  }
</style>
