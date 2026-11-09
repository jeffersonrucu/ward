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
  import Botao from '../lib/componentes/Botao.svelte';
  import BadgeAmbiente from '../lib/componentes/BadgeAmbiente.svelte';
  import BadgeTipo from '../lib/componentes/BadgeTipo.svelte';
  import CabecalhoDeGrupo from '../lib/componentes/CabecalhoDeGrupo.svelte';
  import Campo from '../lib/componentes/Campo.svelte';
  import CampoSecreto from '../lib/componentes/CampoSecreto.svelte';
  import CartaoDeNivel from '../lib/componentes/CartaoDeNivel.svelte';
  import DialogoDeNivel from '../lib/componentes/DialogoDeNivel.svelte';
  import Destaque from '../lib/componentes/Destaque.svelte';
  import DialogoFiltros from '../lib/componentes/DialogoFiltros.svelte';
  import EscolherTipo from './EscolherTipo.svelte';
  import Formulario from './Formulario.svelte';
  import { TIPOS_FORM } from './formulario.js';
  import Icone from '../lib/componentes/Icone.svelte';
  import ItemDeLista from '../lib/componentes/ItemDeLista.svelte';
  import Toast from '../lib/componentes/Toast.svelte';
  import { TIPOS, tipoDe } from '../lib/componentes/tipos.js';
  import { chamar } from '../lib/ponte/index.js';
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

  /** @type {{ ontravado?: () => void }} */
  let { ontravado = undefined } = $props();

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
    { valor: '', rotulo: 'Todos' },
    ...gruposDe(itens).map((g) => ({ valor: g.nome, rotulo: g.nome })),
  ]);
  const opcoesTipo = [
    { valor: '', rotulo: 'Todos' },
    ...Object.entries(TIPOS).map(([valor, t]) => ({ valor, rotulo: t.rotulo })),
  ];
  const opcoesAmbiente = [
    { valor: '', rotulo: 'Todos' },
    ...AMBIENTES.map((a) => ({ valor: a, rotulo: a })),
  ];

  async function carregar() {
    erro = '';
    try {
      itens = await chamar('listar_credenciais');
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
      const d = await chamar('detalhe_credencial', { entrada: nome });
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
      await chamar('copiar_campo', { entrada: detalhe.nome, campo });
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
      const r = await chamar('revelar_campo', { entrada, campo });
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
    filtros = { ...f };
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
      <Campo
        id="busca"
        rotulo="Buscar"
        rotuloOculto
        pilula
        icone="busca"
        placeholder="Buscar por nome, host ou usuário"
        bind:valor={busca}
        bind:entrada={entradaBusca}
        desabilitado={vazio}
        tipo="search"
        aria-keyshortcuts="Control+K /"
        onkeydown={teclaNaBusca}
      />
    </div>
    <Botao
      variante="secundario"
      icone="filtro"
      desabilitado={vazio}
      aria-haspopup="dialog"
      onclick={() => (dialogoAberto = true)}
    >
      {nFiltros > 0 ? `Filtros · ${nFiltros}` : 'Filtrar'}
    </Botao>
  </header>

  <div class="colunas">
    <section class="lista" aria-label="Entradas">
      {#if filtrando && !vazio}
        <div class="filtros-ativos">
          <span class="resumo-filtros">{resumoDosFiltros(filtros, busca)}</span>
          <Botao variante="fantasma" onclick={limparTudo}>Limpar</Botao>
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
                <CabecalhoDeGrupo
                  id="grupo-{g.cor}-{g.nome}"
                  grupo={g.nome}
                  contagem={g.contagem}
                  cor={g.cor}
                />
                {#each g.itens as item (item.nome)}
                  <ItemDeLista
                    compacta
                    nome={item.nome}
                    tipo={item.tipo}
                    ambiente={item.ambiente}
                    detalhe={item.resumo}
                    selecionado={item.nome === selecionada}
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
      {#if erro}<Toast tipo="erro" mensagem={erro} />{/if}
      {#if sucesso}<Toast tipo="sucesso" mensagem={sucesso} />{/if}
      <div class="nova">
        <Botao variante="primario" icone="mais" onclick={novaCredencial}>Nova credencial</Botao>
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
            <span class="avatar"><Icone nome="chave" tamanho="lg" /></span>
            <h2 class="titulo-vazio">Nenhuma credencial ainda</h2>
            <p class="texto-vazio">
              Crie a primeira entrada em Nova credencial. Cada entrada tem um tipo, um grupo e um
              ambiente (dev, stage ou prod).
            </p>
            <Destaque icone="cadeado">
              Entradas novas começam com o nível com aprovação: cada uso do Claude abre a janela
              de aprovação, que pede o seu PIN. Você pode mudar o nível depois, também com o PIN.
            </Destaque>
          </div>
        </div>
      {:else if detalhe}
        <header class="cabecalho">
          <span class="avatar"><Icone nome={tipoDe(detalhe.tipo).icone} tamanho="lg" /></span>
          <div class="identificacao">
            <h2 class="nome">{detalhe.nome}</h2>
            <div class="selos">
              <BadgeTipo tipo={detalhe.tipo} />
              <BadgeAmbiente ambiente={detalhe.ambiente} />
              <span class="subtitulo">{detalhe.subtitulo}</span>
            </div>
          </div>
          <Botao
            variante="secundario"
            desabilitado={!editavel}
            onclick={editar}>Editar</Botao
          >
        </header>
        <div class="corpo">
          {#if detalhe.campos.length > 0}
            <div class="grade">
              {#each detalhe.campos as c (c.id)}
                <Campo
                  id="campo-{c.id}"
                  rotulo={c.rotulo}
                  valor={c.valor}
                  somenteLeitura
                  copiado={copiado === c.id}
                  oncopiar={() => copiar(c.id)}
                />
              {/each}
            </div>
          {/if}
          {#each detalhe.segredos as s (s.id)}
            <CampoSecreto
              id="segredo-{s.id}"
              rotulo={s.rotulo}
              revelado={revelado.id === s.id ? revelado.valor : null}
              copiado={copiado === s.id}
              onrevelar={() => revelar(s.id)}
              onesconder={esconder}
              oncopiar={() => copiar(s.id)}
            />
          {/each}
          {#if detalhe.extras.length > 0}
            <div class="grade">
              {#each detalhe.extras as x (x.nome)}
                {#if x.segredo}
                  <CampoSecreto
                    id="extra-{x.nome}"
                    rotulo={x.nome}
                    ajuda=""
                    revelado={revelado.id === x.nome ? revelado.valor : null}
                    copiado={copiado === x.nome}
                    onrevelar={() => revelar(x.nome)}
                    onesconder={esconder}
                    oncopiar={() => copiar(x.nome)}
                  />
                {:else}
                  <Campo
                    id="extra-{x.nome}"
                    rotulo={x.nome}
                    valor={x.valor ?? ''}
                    somenteLeitura
                    copiado={copiado === x.nome}
                    oncopiar={() => copiar(x.nome)}
                  />
                {/if}
              {/each}
            </div>
          {/if}
          <CartaoDeNivel nivel={detalhe.nivel} onmudar={mudarNivel} />
        </div>
        <footer class="rodape">
          <span class="metadados">{metadados}</span>
          <Botao variante="secundario" icone="copiar">Duplicar</Botao>
          <Botao variante="perigo">Excluir</Botao>
        </footer>
      {:else if carregado && ordem.length === 0}
        <div class="corpo centro">
          <div class="vazio">
            <span class="avatar"><Icone nome="busca" tamanho="lg" /></span>
            <h2 class="titulo-vazio">Nenhuma entrada encontrada</h2>
            <p class="texto-vazio">Limpe a busca ou os filtros para ver todas as entradas.</p>
          </div>
        </div>
      {/if}
    </section>
  </div>
</main>

<DialogoFiltros
  aberto={dialogoAberto}
  {filtros}
  grupos={opcoesGrupo}
  tipos={opcoesTipo}
  ambientes={opcoesAmbiente}
  onaplicar={aplicarFiltros}
  onfechar={() => (dialogoAberto = false)}
/>

<DialogoDeNivel
  aberto={nivelAberto}
  entrada={detalhe}
  onfechar={() => (nivelAberto = false)}
  onmudado={nivelMudado}
  ontravado={cofreTravou}
/>

<style>
  .principal {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  .topo {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-4) var(--espaco-8) var(--espaco-0);
  }
  .titulo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-xl);
    line-height: var(--altura-linha-xl);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .resumo {
    margin: var(--espaco-0);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .busca {
    width: var(--largura-busca);
  }
  .colunas {
    display: flex;
    gap: var(--espaco-16);
    flex: 1;
    min-height: 0;
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
    flex: none;
    width: var(--largura-coluna-lista);
    padding: var(--espaco-16);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
  }
  .filtros-ativos {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  .resumo-filtros {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .rolagem {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .itens {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
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
    gap: var(--espaco-4);
  }
  .lista-vazia {
    margin: var(--espaco-0);
    text-align: center;
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  /* `Esmaecer rolagem` (18948): gradiente fundo/sutil-transparente → fundo/sutil. */
  .esmaecer {
    position: absolute;
    left: var(--espaco-0);
    right: var(--espaco-0);
    bottom: var(--espaco-0);
    height: var(--espaco-48);
    background: linear-gradient(var(--cor-fundo-sutil-transparente), var(--cor-fundo-sutil));
    pointer-events: none;
  }
  .nova {
    display: flex;
  }
  .nova :global(.botao) {
    flex: 1;
  }
  .detalhe {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
    overflow: hidden;
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-24);
    border-bottom: var(--linha-fina) solid var(--cor-borda-sutil);
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--controle-lg);
    height: var(--controle-lg);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .identificacao {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-6);
    flex: 1;
    min-width: 0;
  }
  .nome {
    margin: var(--espaco-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .selos {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    min-width: 0;
  }
  .subtitulo {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
    flex: 1;
    min-height: 0;
    padding: var(--espaco-24);
    overflow-y: auto;
  }
  .corpo.centro {
    align-items: center;
    justify-content: center;
  }
  .grade {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    column-gap: var(--espaco-16);
    row-gap: var(--espaco-20);
  }
  .grade :global(.campo) {
    min-width: 0;
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
    min-width: var(--espaco-0);
    color: var(--cor-texto-secundario);
    font-weight: var(--peso-regular);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    letter-spacing: var(--espacamento-letra-normal);
  }
  .vazio {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--espaco-16);
    width: var(--largura-coluna-acesso);
    text-align: center;
  }
  .titulo-vazio {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .texto-vazio {
    margin: var(--espaco-0);
    color: var(--cor-texto-secundario);
  }
  .vazio :global(.destaque) {
    align-self: stretch;
    text-align: left;
  }
</style>
