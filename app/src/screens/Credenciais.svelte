<script>
  // Tela Credenciais (Fase 32): frames `Credenciais / Vazia` (4780), `/ Cheia` (4063),
  // `/ Filtrada` (4410) e `/ Filtros` (5259) do Ward.figcraft.
  //
  // Dentro da casca do App.svelte (janela 1280×800, padding e gap espaco/16, com a
  // `Navegação lateral`): `principal` (gap espaco/16) com `topo` (padding espaco/4 espaco/8 0, gap espaco/16: título e resumo,
  // busca em pílula de largura/busca, "Filtrar") e `colunas` (gap espaco/16): `lista`
  // (largura/coluna-lista, padding e gap espaco/16 e espaco/12) e `detalhe` (cabeçalho,
  // corpo e rodapé).
  //
  // S11/D4: a lista e o detalhe vêm do Rust sem segredo. O `Campo secreto` desenha a
  // máscara; "revelar" pede `revelar_campo` (D65 b) e o valor sai em 30 s, ao esconder,
  // ao trocar de entrada ou ao sair; "copiar" pede ao Rust, que põe o valor na área de
  // transferência (a resposta não traz o valor). Nada daqui vai para armazenamento do
  // navegador ou URL.
  //
  // Fase 33: "Nova credencial" abre `Escolher tipo` e o formulário no painel de
  // detalhe; "Editar" abre o formulário da entrada. Fase 34: os 9 tipos. Fase 39b: a
  // entrada com anexos também se edita (os mantidos vão pelo nome; o conteúdo fica no
  // daemon).
  //
  // Fase 35: o cartão do nível é o `Cartão de nível` (18998) e "Mudar nível" abre o
  // `Diálogo de nível` (967), que pede o PIN (S9). Mudou: recarrega a lista e o
  // detalhe. O 5º PIN errado trava o cofre: `ontravado` leva o App à tela de travado.
  //
  // Fase 22d (D65 d): o rodapé do detalhe (`metadados` 4400) mostra "Criada em … ·
  // Alterada em …" com as datas do daemon, no fuso local; sem data, nada.
  import { onMount, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import EnvironmentBadge from '../lib/components/EnvironmentBadge.svelte';
  import TypeBadge from '../lib/components/TypeBadge.svelte';
  import GroupHeader from '../lib/components/GroupHeader.svelte';
  import Field from '../lib/components/Field.svelte';
  import SecretField from '../lib/components/SecretField.svelte';
  import LevelCard from '../lib/components/LevelCard.svelte';
  import LevelDialog from '../lib/components/LevelDialog.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import FiltersDialog from '../lib/components/FiltersDialog.svelte';
  import EscolherTipo from './EscolherTipo.svelte';
  import Formulario from './Formulario.svelte';
  import { TIPOS_FORM } from './formulario.js';
  import Icon from '../lib/components/Icon.svelte';
  import ListItem from '../lib/components/ListItem.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { TYPES, entryType } from '../lib/components/types.js';
  import { call } from '../lib/bridge/index.js';
  import {
    AMBIENTES,
    FILTROS_VAZIOS,
    agrupar,
    filtrar,
    filtrosLigados,
    gruposDe,
    resumoDoTopo,
    resumoDosFiltros,
  } from './credenciais.js';

  /** @type {{ onlocked?: () => void }} */
  let { onlocked: ontravado = undefined } = $props();

  // Quanto tempo o "Copiado" fica na `Ação de campo`.
  const COPIADO_MS = 2000;
  const PRAZO_REVELAR_MS = 30_000;
  const SUCESSO_MS = 4000;

  let itens = $state([]);
  let carregado = $state(false);
  let erro = $state('');
  let busca = $state('');
  let filtros = $state({ ...FILTROS_VAZIOS });
  let dialogoAberto = $state(false);
  let selecionada = $state('');
  let detalhe = $state(null);
  let copiado = $state('');
  let entradaBusca = $state();
  let listaEl = $state();
  let rolagem = $state({ esmaecer: false });
  /** 'detalhe' | 'escolher' | 'formulario' */
  let modo = $state('detalhe');
  let tipoForm = $state('');
  let detalheForm = $state(null);
  let sucesso = $state('');
  /** Valor revelado de um segredo do detalhe (um por vez). */
  let revelado = $state({ id: '', valor: '' });
  let prazoRevelar = 0;
  let prazoSucesso = 0;
  let nivelAberto = $state(false);

  const filtrando = $derived(filtrosLigados(filtros) > 0 || busca.trim() !== '');
  const visiveis = $derived(filtrar(itens, filtros, busca));
  const grupos = $derived(agrupar(itens, visiveis, filtrando));
  const ordem = $derived(grupos.flatMap((g) => g.itens.map((i) => i.nome)));
  const vazio = $derived(carregado && itens.length === 0);
  const resumo = $derived(
    resumoDoTopo(itens.length, gruposDe(itens).length, visiveis.length, filtrando),
  );
  const nFiltros = $derived(filtrosLigados(filtros));

  /** `dd/mm/aaaa` no fuso local, de segundos UTC (D65 d). */
  function data(segundos) {
    if (!Number.isSafeInteger(segundos) || segundos <= 0) return '';
    const d = new Date(segundos * 1000);
    if (Number.isNaN(d.getTime())) return '';
    const dois = (n) => String(n).padStart(2, '0');
    return `${dois(d.getDate())}/${dois(d.getMonth() + 1)}/${d.getFullYear()}`;
  }
  const metadados = $derived.by(() => {
    if (!detalhe) return '';
    const criada = data(detalhe.criada_em);
    const alterada = data(detalhe.alterada_em);
    return [criada && `Criada em ${criada}`, alterada && `Alterada em ${alterada}`]
      .filter(Boolean)
      .join(' · ');
  });

  const opcoesGrupo = $derived([
    { value: '', label: 'Todos' },
    ...gruposDe(itens).map((g) => ({ value: g.nome, label: g.nome })),
  ]);
  const opcoesTipo = [
    { value: '', label: 'Todos' },
    ...Object.entries(TYPES).map(([valor, t]) => ({ value: valor, label: t.label })),
  ];
  const opcoesAmbiente = [
    { value: '', label: 'Todos' },
    ...AMBIENTES.map((a) => ({ value: a, label: a })),
  ];

  async function carregar() {
    erro = '';
    try {
      itens = await call('listar_credenciais');
      carregado = true;
    } catch (e) {
      erro = e.mensagem;
    }
  }

  // Só a resposta do último pedido de detalhe vale (cliques rápidos).
  let pedidoDetalhe = 0;

  async function abrir(nome, focar = true) {
    selecionada = nome;
    copiado = '';
    esconder();
    modo = 'detalhe';
    const meu = ++pedidoDetalhe;
    try {
      const d = await call('detalhe_credencial', { entrada: nome });
      if (meu === pedidoDetalhe) detalhe = d;
    } catch (e) {
      if (meu === pedidoDetalhe) {
        detalhe = null;
        erro = e.mensagem;
      }
    }
    if (focar) {
      await tick();
      itemDe(nome)?.focus();
    }
  }

  let temporizador = null;

  async function copiar(campo) {
    if (!detalhe) return;
    erro = '';
    try {
      await call('copiar_campo', { entrada: detalhe.nome, campo });
      copiado = campo;
      clearTimeout(temporizador);
      temporizador = setTimeout(() => (copiado = ''), COPIADO_MS);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  function esconder() {
    clearTimeout(prazoRevelar);
    revelado = { id: '', valor: '' };
  }

  async function revelar(campo) {
    if (!detalhe) return;
    const entrada = detalhe.nome;
    try {
      const r = await call('revelar_campo', { entrada, campo });
      if (detalhe?.nome !== entrada || modo !== 'detalhe') return;
      revelado = { id: campo, valor: r.valor };
      clearTimeout(prazoRevelar);
      prazoRevelar = setTimeout(esconder, PRAZO_REVELAR_MS);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  const editavel = $derived(!!detalhe && Object.hasOwn(TIPOS_FORM, detalhe.tipo));

  function novaCredencial() {
    esconder();
    erro = '';
    modo = 'escolher';
  }

  function editar() {
    if (!editavel) return;
    esconder();
    erro = '';
    tipoForm = detalhe.tipo;
    detalheForm = detalhe;
    modo = 'formulario';
  }

  function cancelarFormulario() {
    modo = 'detalhe';
  }

  async function salvo(nome) {
    modo = 'detalhe';
    await carregar();
    await abrir(nome, false);
    sucesso = 'Credencial salva.';
    clearTimeout(prazoSucesso);
    prazoSucesso = setTimeout(() => (sucesso = ''), SUCESSO_MS);
  }

  function mudarNivel() {
    if (!detalhe) return;
    esconder();
    erro = '';
    nivelAberto = true;
  }

  async function nivelMudado() {
    const nome = detalhe?.nome;
    nivelAberto = false;
    if (!nome) return;
    await carregar();
    await abrir(nome, false);
    sucesso = 'Nível de acesso alterado.';
    clearTimeout(prazoSucesso);
    prazoSucesso = setTimeout(() => (sucesso = ''), SUCESSO_MS);
  }

  function cofreTravou() {
    nivelAberto = false;
    esconder();
    detalhe = null;
    itens = [];
    ontravado?.();
  }

  function itemDe(nome) {
    return listaEl?.querySelector(`[data-entrada="${CSS.escape(nome)}"]`);
  }

  // Setas navegam entre os itens visíveis (só o foco); Enter (ou clique) abre.
  function teclaNaLista(e) {
    const atual = e.target?.closest?.('[data-entrada]')?.dataset.entrada;
    if (atual === undefined) return;
    const k = ordem.indexOf(atual);
    let alvo = null;
    if (e.key === 'ArrowDown') alvo = ordem[Math.min(k + 1, ordem.length - 1)];
    else if (e.key === 'ArrowUp') alvo = ordem[Math.max(k - 1, 0)];
    else if (e.key === 'Home') alvo = ordem[0];
    else if (e.key === 'End') alvo = ordem[ordem.length - 1];
    if (alvo !== null && alvo !== undefined) {
      e.preventDefault();
      itemDe(alvo)?.focus();
      itemDe(alvo)?.scrollIntoView({ block: 'nearest' });
    }
  }

  function teclaNaBusca(e) {
    if (e.key === 'ArrowDown' && ordem.length > 0) {
      e.preventDefault();
      itemDe(ordem[0])?.focus();
    } else if (e.key === 'Enter' && ordem.length > 0) {
      e.preventDefault();
      abrir(ordem[0]);
    } else if (e.key === 'Escape' && busca) {
      e.preventDefault();
      busca = '';
    }
  }

  // Atalho da busca: Ctrl+K em qualquer lugar, ou "/" fora de um campo de texto.
  function teclaGlobal(e) {
    if (dialogoAberto || nivelAberto || vazio) return;
    const emCampo = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
    if ((e.ctrlKey && (e.key === 'k' || e.key === 'K')) || (e.key === '/' && !emCampo)) {
      e.preventDefault();
      entradaBusca?.focus();
      entradaBusca?.select();
    }
  }

  function limparTudo() {
    filtros = { ...FILTROS_VAZIOS };
    busca = '';
  }

  function aplicarFiltros(f) {
    filtros = { grupo: f.group, tipo: f.kind, ambiente: f.environment };
    dialogoAberto = false;
  }

  function medirRolagem() {
    if (!listaEl) return;
    const { scrollTop, scrollHeight, clientHeight } = listaEl;
    rolagem = { esmaecer: scrollTop + clientHeight < scrollHeight - 1 };
  }

  // Com filtros novos, abre a primeira entrada visível se a selecionada sumiu.
  $effect(() => {
    if (!carregado) return;
    if (ordem.length === 0) {
      selecionada = '';
      detalhe = null;
    } else if (!ordem.includes(selecionada)) {
      abrir(ordem[0], false);
    }
  });

  $effect(() => {
    void ordem;
    tick().then(medirRolagem);
  });

  onMount(() => {
    carregar();
    return () => {
      clearTimeout(temporizador);
      clearTimeout(prazoSucesso);
      esconder();
    };
  });
</script>

<svelte:window onkeydown={teclaGlobal} onresize={medirRolagem} />

<main class="principal" aria-labelledby="titulo-credenciais">
  <header class="topo">
    <div class="titulo">
      <h1 id="titulo-credenciais">Credenciais</h1>
      <p class="resumo">{resumo}</p>
    </div>
    <div class="busca">
      <Field
        id="busca"
        label="Buscar"
        hiddenLabel
        pill
        icon="search"
        placeholder="Buscar por nome, host ou usuário"
        bind:value={busca}
        bind:input={entradaBusca}
        disabled={vazio}
        type="search"
        aria-keyshortcuts="Control+K /"
        onkeydown={teclaNaBusca}
      />
    </div>
    <Button
      variant="secondary"
      icon="filter"
      disabled={vazio}
      aria-haspopup="dialog"
      onclick={() => (dialogoAberto = true)}
    >
      {nFiltros > 0 ? `Filtros · ${nFiltros}` : 'Filtrar'}
    </Button>
  </header>

  <div class="colunas">
    <section class="lista" aria-label="Entradas">
      {#if filtrando && !vazio}
        <div class="filtros-ativos">
          <span class="resumo-filtros">{resumoDosFiltros(filtros, busca)}</span>
          <Button variant="ghost" onclick={limparTudo}>Limpar</Button>
        </div>
      {/if}
      <div class="rolagem">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="itens"
          class:centro={vazio || (carregado && ordem.length === 0)}
          bind:this={listaEl}
          onscroll={medirRolagem}
          onkeydown={teclaNaLista}
        >
          {#if vazio}
            <p class="lista-vazia">As entradas aparecem aqui, separadas por grupo.</p>
          {:else if carregado && ordem.length === 0}
            <p class="lista-vazia">Nenhuma entrada com essa busca ou esses filtros.</p>
          {:else}
            {#each grupos as g (g.nome)}
              <div class="grupo" role="group" aria-labelledby="grupo-{g.cor}-{g.nome}">
                <GroupHeader
                  id="grupo-{g.cor}-{g.nome}"
                  group={g.nome}
                  count={g.contagem}
                  color={g.cor}
                />
                {#each g.itens as item (item.nome)}
                  <ListItem
                    compact
                    name={item.nome}
                    kind={item.tipo}
                    environment={item.ambiente}
                    detail={item.resumo}
                    selected={item.nome === selecionada}
                    tabindex={item.nome === selecionada ? 0 : -1}
                    data-entrada={item.nome}
                    onclick={() => abrir(item.nome)}
                  />
                {/each}
              </div>
            {/each}
          {/if}
        </div>
        {#if rolagem.esmaecer}<div class="esmaecer" aria-hidden="true"></div>{/if}
      </div>
      {#if erro}<Toast kind="error" message={erro} />{/if}
      {#if sucesso}<Toast kind="success" message={sucesso} />{/if}
      <div class="nova">
        <Button variant="primary" icon="plus" onclick={novaCredencial}>Nova credencial</Button>
      </div>
    </section>

    <section class="detalhe" aria-label="Detalhe da entrada" aria-live="polite">
      {#if modo === 'escolher'}
        <EscolherTipo
          onescolher={(t) => {
            tipoForm = t;
            detalheForm = null;
            modo = 'formulario';
          }}
          oncancelar={cancelarFormulario}
        />
      {:else if modo === 'formulario'}
        {#key detalheForm?.nome ?? tipoForm}
          <Formulario
            tipo={tipoForm}
            detalhe={detalheForm}
            grupos={gruposDe(itens).map((g) => g.nome)}
            servidores={itens.filter((i) => i.tipo === 'servidor').map((i) => i.nome)}
            oncancelar={cancelarFormulario}
            onsalvo={salvo}
          />
        {/key}
      {:else if vazio}
        <div class="corpo centro">
          <div class="vazio">
            <span class="avatar"><Icon name="key" size="lg" /></span>
            <h2 class="titulo-vazio">Nenhuma credencial ainda</h2>
            <p class="texto-vazio">
              Crie a primeira entrada em Nova credencial. Cada entrada tem um tipo, um grupo e um
              ambiente (dev, stage ou prod).
            </p>
            <Highlight icon="lock">
              Entradas novas começam com o nível com aprovação: cada uso do Claude abre a janela
              de aprovação, que pede o seu PIN. Você pode mudar o nível depois, também com o PIN.
            </Highlight>
          </div>
        </div>
      {:else if detalhe}
        <header class="cabecalho">
          <span class="avatar"><Icon name={entryType(detalhe.tipo).icon} size="lg" /></span>
          <div class="identificacao">
            <h2 class="nome">{detalhe.nome}</h2>
            <div class="selos">
              <TypeBadge kind={detalhe.tipo} />
              <EnvironmentBadge environment={detalhe.ambiente} />
              <span class="subtitulo">{detalhe.subtitulo}</span>
            </div>
          </div>
          <Button
            variant="secondary"
            disabled={!editavel}
            onclick={editar}>Editar</Button
          >
        </header>
        <div class="corpo">
          {#if detalhe.campos.length > 0}
            <div class="grade">
              {#each detalhe.campos as c (c.id)}
                <Field
                  id="campo-{c.id}"
                  label={c.rotulo}
                  value={c.valor}
                  readOnly
                  copied={copiado === c.id}
                  oncopy={() => copiar(c.id)}
                />
              {/each}
            </div>
          {/if}
          {#each detalhe.segredos as s (s.id)}
            <SecretField
              id="segredo-{s.id}"
              label={s.rotulo}
              revealed={revelado.id === s.id ? revelado.valor : null}
              copied={copiado === s.id}
              onreveal={() => revelar(s.id)}
              onhide={esconder}
              oncopy={() => copiar(s.id)}
            />
          {/each}
          {#if detalhe.extras.length > 0}
            <div class="grade">
              {#each detalhe.extras as x (x.nome)}
                {#if x.segredo}
                  <SecretField
                    id="extra-{x.nome}"
                    label={x.nome}
                    help=""
                    revealed={revelado.id === x.nome ? revelado.valor : null}
                    copied={copiado === x.nome}
                    onreveal={() => revelar(x.nome)}
                    onhide={esconder}
                    oncopy={() => copiar(x.nome)}
                  />
                {:else}
                  <Field
                    id="extra-{x.nome}"
                    label={x.nome}
                    value={x.valor ?? ''}
                    readOnly
                    copied={copiado === x.nome}
                    oncopy={() => copiar(x.nome)}
                  />
                {/if}
              {/each}
            </div>
          {/if}
          <LevelCard level={detalhe.nivel} onchange={mudarNivel} />
        </div>
        <footer class="rodape">
          <span class="metadados">{metadados}</span>
          <Button variant="secondary" icon="copy">Duplicar</Button>
          <Button variant="danger">Excluir</Button>
        </footer>
      {:else if carregado && ordem.length === 0}
        <div class="corpo centro">
          <div class="vazio">
            <span class="avatar"><Icon name="search" size="lg" /></span>
            <h2 class="titulo-vazio">Nenhuma entrada encontrada</h2>
            <p class="texto-vazio">Limpe a busca ou os filtros para ver todas as entradas.</p>
          </div>
        </div>
      {/if}
    </section>
  </div>
</main>

<FiltersDialog
  open={dialogoAberto}
  filters={{ group: filtros.grupo, kind: filtros.tipo, environment: filtros.ambiente }}
  groups={opcoesGrupo}
  kinds={opcoesTipo}
  environments={opcoesAmbiente}
  onapply={aplicarFiltros}
  onclose={() => (dialogoAberto = false)}
/>

<LevelDialog
  open={nivelAberto}
  entry={detalhe}
  onclose={() => (nivelAberto = false)}
  onchanged={nivelMudado}
  onlocked={cofreTravou}
/>

<style>
  .principal {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  .topo {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-4) var(--space-8) var(--space-0);
  }
  .titulo {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .resumo {
    margin: var(--space-0);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .busca {
    width: var(--width-search);
  }
  .colunas {
    display: flex;
    gap: var(--space-16);
    flex: 1;
    min-height: 0;
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    flex: none;
    width: var(--width-column-list);
    padding: var(--space-16);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .filtros-ativos {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  .resumo-filtros {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .rolagem {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .itens {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    height: 100%;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .itens.centro {
    justify-content: center;
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .lista-vazia {
    margin: var(--space-0);
    text-align: center;
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  /* `Esmaecer rolagem` (18948): gradiente fundo/sutil-transparente → fundo/sutil. */
  .esmaecer {
    position: absolute;
    left: var(--space-0);
    right: var(--space-0);
    bottom: var(--space-0);
    height: var(--space-48);
    background: linear-gradient(var(--color-bg-subtle-transparent), var(--color-bg-subtle));
    pointer-events: none;
  }
  .nova {
    display: flex;
  }
  .nova :global(.button) {
    flex: 1;
  }
  .detalhe {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow: hidden;
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .identificacao {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    flex: 1;
    min-width: 0;
  }
  .nome {
    margin: var(--space-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .selos {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }
  .subtitulo {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    min-height: 0;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .corpo.centro {
    align-items: center;
    justify-content: center;
  }
  .grade {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    column-gap: var(--space-16);
    row-gap: var(--space-20);
  }
  .grade :global(.field) {
    min-width: 0;
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
    min-width: var(--space-0);
    color: var(--color-text-secondary);
    font-weight: var(--weight-regular);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    letter-spacing: var(--letter-spacing-normal);
  }
  .vazio {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-16);
    width: var(--width-column-access);
    text-align: center;
  }
  .titulo-vazio {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .texto-vazio {
    margin: var(--space-0);
    color: var(--color-text-secondary);
  }
  .vazio :global(.highlight) {
    align-self: stretch;
    text-align: left;
  }
</style>
