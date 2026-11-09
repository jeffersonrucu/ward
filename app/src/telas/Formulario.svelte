<script>
  // Formulário de credencial (Fases 33 e 34): frames `Formulário / <tipo> / Novo` e
  // `/ Edição` dos 9 tipos (ids em docs/mission/design-nodes.md). Ocupa o painel de detalhe de Credenciais: cabeçalho
  // (avatar, nome ou "Nova credencial", selos), corpo com rolagem (identificação em grade
  // de 2 colunas, "Acesso" com os campos do tipo, "Campos extras", "Anexos", cartão do
  // nível) e rodapé (obrigatórios, Cancelar, Salvar).
  //
  // S11/D4/D65: o valor salvo nunca chega aqui. Segredo salvo aparece como máscara e vai
  // no salvar como "manter"; "revelar" pede `revelar_campo` e o valor sai em 30 s, ao
  // esconder ou ao sair; "copiar" pede ao Rust. A senha do gerador fica no Rust ("Usar
  // esta senha" devolve só o tamanho) e vai como "gerada". O que o usuário digita fica
  // só no estado desta tela, que some ao sair (`descartar_formulario` limpa o Rust).
  //
  // Fase 34: anexos são escolhidos e lidos no Rust (`escolher_anexo`); a tela recebe só
  // id, nome e tamanho, ou o aviso (arquivo acima de 256 KiB, soma acima disso, vazio).
  // "Conectar" (D3) pede ao Rust o token e o terminal; o token não volta.
  import { onDestroy, onMount } from 'svelte';
  import Anexo from '../lib/componentes/Anexo.svelte';
  import BadgeAmbiente from '../lib/componentes/BadgeAmbiente.svelte';
  import BadgeTipo from '../lib/componentes/BadgeTipo.svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import Campo from '../lib/componentes/Campo.svelte';
  import CampoExtra from '../lib/componentes/CampoExtra.svelte';
  import CampoSecreto from '../lib/componentes/CampoSecreto.svelte';
  import CodigoTotp from '../lib/componentes/CodigoTotp.svelte';
  import ComboboxGrupo from '../lib/componentes/ComboboxGrupo.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import PopoverGerador from '../lib/componentes/PopoverGerador.svelte';
  import Select from '../lib/componentes/Select.svelte';
  import Toast from '../lib/componentes/Toast.svelte';
  import TituloDeSecao from '../lib/componentes/TituloDeSecao.svelte';
  import { tipoDe } from '../lib/componentes/tipos.js';
  import { chamar } from '../lib/ponte/index.js';
  import { NIVEIS } from './credenciais.js';
  import {
    AMBIENTES_FORM,
    AUTENTICACOES,
    MOTORES,
    PROTOCOLOS,
    REGIOES_AWS,
    SEGURANCAS,
    TEXTOS_ANEXOS,
    TEXTOS_EXTRAS,
    TIPOS_FORM,
    acessoDoBanco,
    campoVisivel,
    camposDoTipo,
    errosPorCampo,
    montarFormulario,
    novoVazio,
    portaPadrao,
    tamanhoLegivel,
    variavelDoAnexo,
  } from './formulario.js';

  const PRAZO_REVELAR_MS = 30_000;
  const COPIADO_MS = 2_000;

  /**
   * @type {{ tipo: string, detalhe?: any, grupos: string[], servidores?: string[],
   *   oncancelar: () => void, onsalvo: (nome: string) => void }}
   */
  let { tipo, detalhe = null, grupos, servidores = [], oncancelar, onsalvo } = $props();

  const editando = detalhe !== null;
  const def = TIPOS_FORM[tipo];
  const camposTipo = camposDoTipo(tipo);

  const inicial = (id) => detalhe?.campos.find((c) => c.id === id)?.valor ?? '';
  const salvo = (id) => detalhe?.segredos.some((s) => s.id === id) ?? false;

  let nome = $state(detalhe?.nome ?? '');
  let grupo = $state(detalhe?.grupo ?? '');
  let ambiente = $state(detalhe?.ambiente ?? '');
  let projeto = $state(detalhe?.projeto ?? '');
  let tags = $state((detalhe?.tags ?? []).join(', '));
  // Valor inicial dos selects sem valor salvo. "Autenticação" (só da tela) vem do
  // segredo que a entrada editada tem.
  const PADROES = {
    seguranca: 'starttls',
    motor: 'postgres',
    protocolo: 'sftp',
    autenticacao: salvo('chave_privada') ? 'chave' : 'senha',
  };
  let campos = $state(
    Object.fromEntries(
      camposTipo
        .filter((c) => c.tipo === 'texto' || c.tipo === 'select')
        .map((c) => [c.id, inicial(c.id) || (PADROES[c.id] ?? '')]),
    ),
  );
  let segredos = $state(
    Object.fromEntries(
      camposTipo
        .filter((c) => c.tipo === 'segredo' || c.tipo === 'nota')
        .map((c) => [c.id, { modo: salvo(c.id) ? 'salvo' : 'digitando', digitado: '', tamanho: 0 }]),
    ),
  );
  let proximoExtra = 0;
  let extras = $state(
    (detalhe?.extras ?? []).map((x) => ({
      chave: proximoExtra++,
      nome: x.nome,
      valor: x.segredo ? '' : (x.valor ?? ''),
      segredo: x.segredo,
      guardado: x.segredo,
      original: x.nome,
    })),
  );

  /** @type {Array<{ id: number, nome: string, tamanho: number }>} */
  let anexos = $state([]);
  // V5.10 (Fase 39c): no Novo, "Salvar" fica desabilitado até algo ser preenchido.
  const camposIniciais = $state.snapshot(campos);
  let vazio = $derived(
    !editando &&
      novoVazio(
        { nome, grupo, ambiente, projeto, tags, campos, segredos, extras, anexos },
        camposIniciais,
      ),
  );
  let avisoAnexo = $state('');
  let escolhendo = $state(false);
  let conectando = $state(false);
  let conectado = $state(false);

  let erros = $state({});
  let erroGeral = $state('');
  let salvando = $state(false);
  /** @type {Record<string, string>} */
  let revelados = $state({});
  let copiado = $state('');
  let geradorPara = $state('');
  /** @type {{ codigo: string, restante: number } | null} */
  let totp = $state(null);

  const nivel = $derived(NIVEIS[detalhe?.nivel] ?? NIVEIS['com-aprovacao']);
  const regioes = $derived(
    [{ valor: '', rotulo: 'Escolha a região' }].concat(
      [...new Set([...(campos.regiao ? [campos.regiao] : []), ...REGIOES_AWS])].map((r) => ({
        valor: r,
        rotulo: r,
      })),
    ),
  );
  // Servidores para túnel e salto: os do cofre, menos a própria entrada; o valor salvo
  // entra na lista mesmo que a entrada não exista mais (a validação dá o erro).
  const opcoesServidores = (atual) =>
    [{ valor: '', rotulo: 'Nenhum' }].concat(
      [...new Set([...(atual ? [atual] : []), ...servidores])]
        .filter((n) => n !== detalhe?.nome)
        .map((n) => ({ valor: n, rotulo: n })),
    );
  const opcoesSelect = $derived({
    regioes,
    segurancas: SEGURANCAS,
    motores: MOTORES,
    protocolos: PROTOCOLOS,
    // ftp não aceita chave (o ward-core recusa); a tela só oferece senha.
    autenticacoes: campos.protocolo === 'ftp' ? AUTENTICACOES.slice(0, 1) : AUTENTICACOES,
    servidores: opcoesServidores(campos.tunel),
    saltos: opcoesServidores(campos.salto),
  });
  const textoAcesso = $derived(tipo === 'banco' ? acessoDoBanco(campos.motor) : def.acesso);
  const linhasVisiveis = $derived(
    def.linhas
      .map((l) => l.filter((c) => campoVisivel(c, campos.autenticacao)))
      .filter((l) => l.length > 0),
  );

  $effect(() => {
    if (campos.protocolo === 'ftp' && campos.autenticacao !== 'senha') campos.autenticacao = 'senha';
  });

  let ativa = true;
  const prazos = {};
  let prazoCopiado = 0;
  let prazoTotp = 0;

  function esconder(id) {
    clearTimeout(prazos[id]);
    delete revelados[id];
  }

  function esconderTudo() {
    for (const id of Object.keys(revelados)) esconder(id);
  }

  async function revelar(id) {
    if (!editando) return;
    try {
      const r = await chamar('revelar_campo', { entrada: detalhe.nome, campo: id });
      if (!ativa) return;
      revelados[id] = r.valor;
      clearTimeout(prazos[id]);
      prazos[id] = setTimeout(() => esconder(id), PRAZO_REVELAR_MS);
    } catch (e) {
      erroGeral = e.mensagem;
    }
  }

  function marcarCopiado(id) {
    copiado = id;
    clearTimeout(prazoCopiado);
    prazoCopiado = setTimeout(() => (copiado = ''), COPIADO_MS);
  }

  async function copiar(id) {
    if (!editando) return;
    try {
      await chamar('copiar_campo', { entrada: detalhe.nome, campo: id });
      marcarCopiado(id);
    } catch (e) {
      erroGeral = e.mensagem;
    }
  }

  async function copiarTotp() {
    try {
      await chamar('copiar_codigo_totp', { entrada: detalhe.nome });
      marcarCopiado('codigo');
    } catch (e) {
      erroGeral = e.mensagem;
    }
  }

  // Código TOTP da entrada editada: um pedido, contagem local, novo pedido no fim.
  async function atualizarTotp() {
    clearTimeout(prazoTotp);
    if (!editando || !salvo('totp_semente') || segredos.totp_semente?.modo !== 'salvo') {
      totp = null;
      return;
    }
    try {
      const r = await chamar('codigo_totp', { entrada: detalhe.nome });
      if (!ativa) return;
      totp = r;
      contar();
    } catch {
      totp = null;
    }
  }

  function contar() {
    prazoTotp = setTimeout(() => {
      if (!ativa || !totp) return;
      if (totp.restante <= 1) atualizarTotp();
      else {
        totp = { ...totp, restante: totp.restante - 1 };
        contar();
      }
    }, 1000);
  }

  function trocar(id) {
    esconder(id);
    segredos[id] = { modo: 'digitando', digitado: '', tamanho: 0 };
    if (id === 'totp_semente') {
      clearTimeout(prazoTotp);
      totp = null;
    }
  }

  function usarGerada(tamanho) {
    segredos[geradorPara] = { modo: 'gerada', digitado: '', tamanho };
    geradorPara = '';
  }

  function adicionarExtra() {
    extras.push({
      chave: proximoExtra++,
      nome: '',
      valor: '',
      segredo: false,
      guardado: false,
      original: '',
    });
  }

  async function adicionarAnexo() {
    if (escolhendo) return;
    escolhendo = true;
    avisoAnexo = '';
    try {
      const r = await chamar('escolher_anexo');
      if (!ativa) return;
      if (r.resultado === 'anexado') {
        anexos.push({ id: r.id, nome: r.nome, tamanho: r.tamanho });
        delete erros.anexos;
      } else if (r.resultado === 'aviso') {
        avisoAnexo = r.motivo;
      }
    } catch (e) {
      avisoAnexo = e.mensagem;
    } finally {
      escolhendo = false;
    }
  }

  async function removerAnexo(i) {
    const a = anexos[i];
    try {
      await chamar('remover_anexo', { id: a.id });
      anexos.splice(i, 1);
      avisoAnexo = '';
    } catch (e) {
      avisoAnexo = e.mensagem;
    }
  }

  async function conectar() {
    if (conectando || !editando) return;
    conectando = true;
    erroGeral = '';
    try {
      await chamar('conectar', { entrada: detalhe.nome });
      if (!ativa) return;
      conectado = true;
      clearTimeout(prazoCopiado);
      prazoCopiado = setTimeout(() => (conectado = false), COPIADO_MS);
    } catch (e) {
      erroGeral = e.mensagem;
    } finally {
      conectando = false;
    }
  }

  function erroDe(id) {
    return erros[id] ?? '';
  }

  async function salvar() {
    if (salvando || vazio) return;
    salvando = true;
    erroGeral = '';
    esconderTudo();
    try {
      const formulario = montarFormulario(tipo, {
        nome,
        grupo,
        ambiente,
        projeto,
        tags,
        campos,
        segredos,
        extras,
        anexos,
      });
      const r = await chamar('salvar_credencial', {
        anterior: editando ? detalhe.nome : null,
        formulario,
      });
      if (!ativa) return;
      if (r.salvo) {
        onsalvo(r.salvo);
        return;
      }
      erros = errosPorCampo(r.erros);
      erroGeral = erros.formulario ?? '';
    } catch (e) {
      erroGeral = e.mensagem;
    } finally {
      salvando = false;
    }
  }

  onMount(atualizarTotp);

  onDestroy(() => {
    ativa = false;
    esconderTudo();
    clearTimeout(prazoCopiado);
    clearTimeout(prazoTotp);
    chamar('descartar_formulario').catch(() => {});
  });
</script>

{#snippet campoSecreto(c)}
  {@const s = segredos[c.id]}
  <CampoSecreto
    id="form-{c.id}"
    rotulo={c.rotulo}
    editavel
    vazio={s.modo === 'digitando'}
    bind:digitado={segredos[c.id].digitado}
    mascara={s.modo === 'gerada' ? '•'.repeat(s.tamanho) : '••••••••'}
    mostrarRevelar={s.modo === 'salvo'}
    mostrarCopiar={s.modo === 'salvo'}
    mostrarGerar={!!c.gerar}
    revelado={revelados[c.id] ?? null}
    copiado={copiado === c.id}
    erro={erroDe(c.id)}
    ajuda={s.modo === 'salvo'
      ? 'Revelar e copiar não pedem PIN.'
      : s.modo === 'gerada'
        ? 'Senha gerada. Ela fica oculta e vai para o cofre ao salvar.'
        : 'Digite ou gere o valor. Depois de salvo, ele fica oculto.'}
    placeholder="Nenhum valor salvo"
    onrevelar={() => revelar(c.id)}
    onesconder={() => esconder(c.id)}
    oncopiar={() => copiar(c.id)}
    ontrocar={() => trocar(c.id)}
    ongerar={() => (geradorPara = c.id)}
  />
{/snippet}

{#snippet campoDaLinha(c)}
  {#if c.tipo === 'texto'}
    <Campo
      id="form-{c.id}"
      rotulo={c.rotulo}
      bind:valor={campos[c.id]}
      placeholder={c.porta ? portaPadrao(tipo, campos) : (c.placeholder ?? '')}
      ajuda={c.ajuda ?? ''}
      erro={erroDe(c.id)}
      inputmode={c.modo}
      oncopiar={editando && inicial(c.id) ? () => copiar(c.id) : undefined}
      copiado={copiado === c.id}
    />
  {:else if c.tipo === 'select'}
    <div class="com-erro-select">
      <Select
        id="form-{c.id}"
        rotulo={c.rotulo}
        bind:valor={campos[c.id]}
        opcoes={opcoesSelect[c.opcoes]}
      />
      {#if erroDe(c.id)}<span class="erro">{erroDe(c.id)}</span>{/if}
    </div>
  {:else if c.tipo === 'segredo' && c.multilinha && segredos[c.id].modo === 'digitando'}
    <!-- Chave privada: várias linhas (o <input> perderia as quebras). O texto fica
         mascarado na tela (text-security) e só no estado deste formulário. -->
    <div class="nota">
      <label class="rotulo" for="form-{c.id}">{c.rotulo}</label>
      <textarea
        id="form-{c.id}"
        class="texto-nota secreto"
        class:com-erro={erroDe(c.id)}
        bind:value={segredos[c.id].digitado}
        placeholder="Cole a chave privada (formato OpenSSH ou PEM)"
        spellcheck="false"
        autocomplete="off"
        aria-invalid={erroDe(c.id) ? 'true' : undefined}
      ></textarea>
      <span class="ajuda" class:erro={erroDe(c.id)}>
        {erroDe(c.id) || 'O conteúdo fica oculto. Depois de salvo, ele só aparece com revelar.'}
      </span>
    </div>
  {:else if c.tipo === 'segredo'}
    {@render campoSecreto(c)}
  {:else if c.tipo === 'totp'}
    <CodigoTotp
      id="form-codigo"
      codigo={totp?.codigo ?? null}
      restante={totp?.restante ?? 0}
      copiado={copiado === 'codigo'}
      oncopiar={copiarTotp}
    />
  {:else if c.tipo === 'nota'}
    {@const s = segredos[c.id]}
    {#if s.modo === 'digitando'}
      <div class="nota">
        <label class="rotulo" for="form-texto">{c.rotulo}</label>
        <textarea
          id="form-texto"
          class="texto-nota"
          class:com-erro={erroDe('texto')}
          bind:value={segredos.texto.digitado}
          placeholder="Escreva a nota. Ela fica cifrada no cofre e só aparece aqui no app."
          spellcheck="false"
          aria-invalid={erroDe('texto') ? 'true' : undefined}
        ></textarea>
        <span class="ajuda" class:erro={erroDe('texto')}>
          {erroDe('texto') || 'Cifrada no cofre'}
        </span>
      </div>
    {:else}
      {@render campoSecreto(c)}
    {/if}
  {/if}
{/snippet}

<header class="cabecalho">
  <span class="avatar"><Icone nome={tipoDe(tipo).icone} tamanho="lg" /></span>
  <div class="identificacao">
    <h2 class="nome">{editando ? detalhe.nome : 'Nova credencial'}</h2>
    <div class="selos">
      <BadgeTipo {tipo} />
      {#if editando}
        <BadgeAmbiente ambiente={detalhe.ambiente} />
        <span class="subtitulo">grupo {detalhe.grupo} · editando</span>
      {:else}
        <span class="subtitulo">preencha nome, grupo e ambiente</span>
      {/if}
    </div>
  </div>
  {#if editando && tipo === 'servidor'}
    <Botao
      variante="secundario"
      icone={conectado ? 'check' : 'tipo-servidor'}
      desabilitado={conectando}
      title="Abre o seu terminal já conectado, sem PIN (token de uso único)."
      data-conectado={conectado ? 'sim' : undefined}
      onclick={conectar}>Conectar</Botao
    >
  {/if}
</header>

<form
  class="corpo"
  id="formulario-credencial"
  novalidate
  onsubmit={(e) => {
    e.preventDefault();
    salvar();
  }}
>
  <div class="grade">
    <Campo
      id="form-nome"
      rotulo="Nome"
      bind:valor={nome}
      placeholder={def.exemplo ?? 'Ex.: acme-wp-dev'}
      erro={erroDe('nome')}
    />
    <Select
      id="form-tipo"
      rotulo="Tipo"
      valor={tipo}
      opcoes={[{ valor: tipo, rotulo: tipoDe(tipo).rotulo }]}
      desabilitado={editando}
    />
    <ComboboxGrupo id="form-grupo" bind:valor={grupo} {grupos} erro={erroDe('grupo')} />
    <div class="com-erro-select">
      <Select id="form-ambiente" rotulo="Ambiente" bind:valor={ambiente} opcoes={AMBIENTES_FORM} />
      {#if erroDe('ambiente')}<span class="erro">{erroDe('ambiente')}</span>{/if}
    </div>
    <Campo
      id="form-projeto"
      rotulo="Projeto"
      bind:valor={projeto}
      placeholder="Opcional"
      erro={erroDe('projeto')}
    />
    <Campo
      id="form-tags"
      rotulo="Tags"
      bind:valor={tags}
      placeholder="Separe por vírgula"
      erro={erroDe('tags')}
    />
  </div>

  {#if def.linhas.length > 0}
    <section class="secao" aria-labelledby="secao-acesso">
      <TituloDeSecao id="secao-acesso" titulo={def.tituloAcesso ?? 'Acesso'} descricao={textoAcesso} />
      {#each linhasVisiveis as linha (linha.map((c) => c.id).join('+'))}
        <div class="linha" class:dupla={linha.length > 1}>
          {#each linha as c (c.id)}
            {@render campoDaLinha(c)}
          {/each}
        </div>
      {/each}
    </section>
  {/if}

  <section class="secao" aria-labelledby="secao-extras">
    <TituloDeSecao
      id="secao-extras"
      titulo="Campos extras"
      descricao={tipo === 'generico' && editando
        ? 'Só eles são entregues, como WARD_<NOME>.'
        : (TEXTOS_EXTRAS[tipo] ?? TEXTOS_EXTRAS.padrao)}
    >
      {#snippet acao()}
        <Botao variante="fantasma" icone="mais" onclick={adicionarExtra}>Adicionar campo</Botao>
      {/snippet}
    </TituloDeSecao>
    {#if extras.length === 0}
      <p class="caixa-vazia" class:erro={erroDe('extras')}>
        {erroDe('extras') || 'Nenhum campo extra. Cada um tem nome, valor e a chave segredo.'}
      </p>
    {:else}
      {#each extras as x, i (x.chave)}
        <CampoExtra
          id="extra-{x.chave}"
          bind:nome={x.nome}
          bind:valor={x.valor}
          bind:segredo={x.segredo}
          guardado={x.guardado}
          erro={erros[`extras-${i + 1}`] ?? ''}
          onremover={() => extras.splice(i, 1)}
          ontrocar={() => {
            x.guardado = false;
            x.valor = '';
          }}
        />
      {/each}
      {#if erroDe('extras')}<span class="erro">{erroDe('extras')}</span>{/if}
    {/if}
  </section>

  <section class="secao" aria-labelledby="secao-anexos">
    <TituloDeSecao
      id="secao-anexos"
      titulo="Anexos"
      descricao={TEXTOS_ANEXOS[tipo] ?? TEXTOS_ANEXOS.padrao}
    >
      {#snippet acao()}
        <Botao variante="fantasma" icone="clipe" desabilitado={escolhendo} onclick={adicionarAnexo}
          >Adicionar arquivo</Botao
        >
      {/snippet}
    </TituloDeSecao>
    {#if anexos.length === 0}
      <p class="caixa-vazia" class:erro={erroDe('anexos')}>
        Nenhum anexo. Use Adicionar arquivo (até 256 KiB somados).
      </p>
    {:else}
      {#each anexos as a, i (a.id)}
        <Anexo
          nome={a.nome}
          detalhe="{tamanhoLegivel(a.tamanho)} · {variavelDoAnexo(a.nome)}"
          onremover={() => removerAnexo(i)}
        />
      {/each}
    {/if}
    {#if avisoAnexo}<span class="erro" role="alert" id="aviso-anexo">{avisoAnexo}</span>{/if}
    {#if erroDe('anexos')}<span class="erro" id="erro-anexos">{erroDe('anexos')}</span>{/if}
  </section>

  <div class="nivel">
    <Icone nome={nivel.icone} />
    <div class="textos-nivel">
      <span class="rotulo-nivel">Nível de acesso</span>
      <span class="nome-nivel">{nivel.rotulo}</span>
      <span class="explicacao">
        {editando
          ? nivel.explicacao
          : 'Toda entrada nova começa assim. Mudar o nível pede o PIN.'}
      </span>
    </div>
    <Botao variante="secundario" icone="cadeado" desabilitado>Mudar nível</Botao>
  </div>
</form>

{#if geradorPara}
  <div class="camada-popover">
    <PopoverGerador campo={geradorPara} onusar={usarGerada} onfechar={() => (geradorPara = '')} />
  </div>
{/if}

<footer class="rodape">
  <span class="metadados">{editando ? '' : 'Obrigatórios: nome, grupo e ambiente.'}</span>
  {#if erroGeral}<Toast tipo="erro" mensagem={erroGeral} />{/if}
  <Botao variante="secundario" onclick={oncancelar}>Cancelar</Botao>
  <Botao
    variante="primario"
    tipo="submit"
    form="formulario-credencial"
    desabilitado={salvando || vazio}
  >
    {editando ? 'Salvar alterações' : 'Salvar'}
  </Botao>
</footer>

<style>
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
  .identificacao {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
    min-width: 0;
  }
  .nome {
    margin: 0;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
  }
  .selos {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  .subtitulo {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subtitulo,
  .metadados {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: var(--espaco-24);
    overflow-y: auto;
  }
  .grade,
  .linha.dupla {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--espaco-16);
  }
  .grade {
    row-gap: var(--espaco-20);
  }
  .grade > :global(*),
  .linha > :global(*) {
    min-width: 0;
  }
  .secao {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
  }
  .linha {
    display: flex;
    flex-direction: column;
  }
  .com-erro-select {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
  .com-erro-select > :global(.select) {
    min-width: 0;
  }
  .nota {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .texto-nota {
    min-height: var(--espaco-96);
    padding: var(--espaco-12) var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
    color: var(--cor-texto-primario);
    font: inherit;
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    resize: vertical;
  }
  .texto-nota:focus-visible {
    outline: none;
    border-color: var(--cor-borda-foco);
  }
  .texto-nota.com-erro {
    border-color: var(--cor-perigo-borda);
  }
  .texto-nota.secreto {
    -webkit-text-security: disc;
  }
  .texto-nota::placeholder {
    color: var(--cor-texto-secundario);
  }
  .ajuda,
  .erro {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .erro,
  .ajuda.erro {
    color: var(--cor-perigo-texto);
  }
  .caixa-vazia {
    margin: 0;
    padding: var(--espaco-16);
    border-radius: var(--raio-item);
    border: var(--linha-fina) dashed var(--cor-borda-padrao);
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    text-align: center;
  }
  .caixa-vazia.erro {
    border-color: var(--cor-perigo-borda);
    color: var(--cor-perigo-texto);
  }
  .nivel {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .textos-nivel {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
  }
  .rotulo-nivel,
  .explicacao {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .nome-nivel {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .explicacao {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .camada-popover {
    position: absolute;
    right: var(--espaco-24);
    bottom: var(--espaco-80);
    z-index: 3;
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
  }
</style>
