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
  //
  // Fase 53a (S2, S11): "Carregar de arquivo" no campo "Chave privada" pede ao Rust
  // (`carregar_chave`), que escolhe, lê e confere o arquivo e guarda a chave; a tela
  // recebe só o tamanho (ou o aviso), e a chave vai no salvar como "carregada", sem
  // área de transferência e sem passar por aqui.
  import { onDestroy, onMount } from 'svelte';
  import Attachment from '../lib/components/Attachment.svelte';
  import EnvironmentBadge from '../lib/components/EnvironmentBadge.svelte';
  import TypeBadge from '../lib/components/TypeBadge.svelte';
  import Button from '../lib/components/Button.svelte';
  import Field from '../lib/components/Field.svelte';
  import ExtraField from '../lib/components/ExtraField.svelte';
  import SecretField from '../lib/components/SecretField.svelte';
  import TotpCode from '../lib/components/TotpCode.svelte';
  import GroupCombobox from '../lib/components/GroupCombobox.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import GeneratorPopover from '../lib/components/GeneratorPopover.svelte';
  import Select from '../lib/components/Select.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import SectionTitle from '../lib/components/SectionTitle.svelte';
  import { entryType } from '../lib/components/types.js';
  import { call } from '../lib/bridge/index.js';
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
  // Fase 39b: anexos já guardados na entrada editada (nome e tamanho do detalhe; o
  // conteúdo nunca vem). Remover aqui só tira da lista; vale ao salvar.
  /** @type {Array<{ nome: string, tamanho: number }>} */
  let guardados = $state((detalhe?.anexos ?? []).map((a) => ({ nome: a.nome, tamanho: a.tamanho })));

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
  /** @type {Record<string, string>} */
  let avisoChave = $state({});
  let carregando = $state(false);
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
    [{ value: '', label: 'Escolha a região' }].concat(
      [...new Set([...(campos.regiao ? [campos.regiao] : []), ...REGIOES_AWS])].map((r) => ({
        value: r,
        label: r,
      })),
    ),
  );
  // Servidores para túnel e salto: os do cofre, menos a própria entrada; o valor salvo
  // entra na lista mesmo que a entrada não exista mais (a validação dá o erro).
  const opcoesServidores = (atual) =>
    [{ value: '', label: 'Nenhum' }].concat(
      [...new Set([...(atual ? [atual] : []), ...servidores])]
        .filter((n) => n !== detalhe?.nome)
        .map((n) => ({ value: n, label: n })),
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
      const r = await call('revelar_campo', { entrada: detalhe.nome, campo: id });
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
      await call('copiar_campo', { entrada: detalhe.nome, campo: id });
      marcarCopiado(id);
    } catch (e) {
      erroGeral = e.mensagem;
    }
  }

  async function copiarTotp() {
    try {
      await call('copiar_codigo_totp', { entrada: detalhe.nome });
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
      const r = await call('codigo_totp', { entrada: detalhe.nome });
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
    delete avisoChave[id];
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

  // Fase 53a: a chave fica no Rust; aqui só o modo "carregada" e o tamanho (máscara).
  async function carregarChave(id) {
    if (carregando) return;
    carregando = true;
    delete avisoChave[id];
    try {
      const r = await call('carregar_chave', { campo: id });
      if (!ativa) return;
      if (r.resultado === 'carregada') {
        segredos[id] = { modo: 'carregada', digitado: '', tamanho: r.tamanho };
        delete erros[id];
      } else if (r.resultado === 'aviso') {
        avisoChave[id] = r.motivo;
      }
    } catch (e) {
      avisoChave[id] = e.mensagem;
    } finally {
      carregando = false;
    }
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
      const r = await call(
        'escolher_anexo',
        editando ? { edicao: { entrada: detalhe.nome, mantidos: guardados.map((a) => a.nome) } } : {},
      );
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

  function removerGuardado(i) {
    guardados.splice(i, 1);
    avisoAnexo = '';
    delete erros.anexos;
  }

  async function removerAnexo(i) {
    const a = anexos[i];
    try {
      await call('remover_anexo', { id: a.id });
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
      await call('conectar', { entrada: detalhe.nome });
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
        guardados,
      });
      const r = await call('salvar_credencial', {
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
    call('descartar_formulario').catch(() => {});
  });
</script>

{#snippet campoSecreto(c)}
  {@const s = segredos[c.id]}
  <SecretField
    id="form-{c.id}"
    label={c.rotulo}
    editable
    empty={s.modo === 'digitando'}
    bind:typed={segredos[c.id].digitado}
    mask={s.modo === 'gerada' ? '•'.repeat(s.tamanho) : '••••••••'}
    showReveal={s.modo === 'salvo'}
    showCopy={s.modo === 'salvo'}
    showGenerate={!!c.gerar}
    revealed={revelados[c.id] ?? null}
    copied={copiado === c.id}
    error={erroDe(c.id)}
    help={s.modo === 'salvo'
      ? 'Revelar e copiar não pedem PIN.'
      : s.modo === 'gerada'
        ? 'Senha gerada. Ela fica oculta e vai para o cofre ao salvar.'
        : s.modo === 'carregada'
          ? `Chave carregada do arquivo (${tamanhoLegivel(s.tamanho)}). Ela fica oculta e vai para o cofre ao salvar.`
          : 'Digite ou gere o valor. Depois de salvo, ele fica oculto.'}
    placeholder="Nenhum valor salvo"
    onreveal={() => revelar(c.id)}
    onhide={() => esconder(c.id)}
    oncopy={() => copiar(c.id)}
    onchange={() => trocar(c.id)}
    ongenerate={() => (geradorPara = c.id)}
  />
{/snippet}

{#snippet campoDaLinha(c)}
  {#if c.tipo === 'texto'}
    <Field
      id="form-{c.id}"
      label={c.rotulo}
      bind:value={campos[c.id]}
      placeholder={c.porta ? portaPadrao(tipo, campos) : (c.placeholder ?? '')}
      help={c.ajuda ?? ''}
      error={erroDe(c.id)}
      inputmode={c.modo}
      oncopy={editando && inicial(c.id) ? () => copiar(c.id) : undefined}
      copied={copiado === c.id}
    />
  {:else if c.tipo === 'select'}
    <div class="com-erro-select">
      <Select
        id="form-{c.id}"
        label={c.rotulo}
        bind:value={campos[c.id]}
        options={opcoesSelect[c.opcoes]}
      />
      {#if erroDe(c.id)}<span class="erro">{erroDe(c.id)}</span>{/if}
    </div>
  {:else if c.tipo === 'segredo' && c.multilinha && segredos[c.id].modo === 'digitando'}
    <!-- Chave privada: várias linhas (o <input> perderia as quebras). O texto fica
         mascarado na tela (text-security) e só no estado deste formulário.
         Fase 53a: "Carregar de arquivo" lê o arquivo no Rust; o conteúdo não vem. -->
    {@const aviso = avisoChave[c.id] ?? ''}
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
      <div class="acao-chave">
        <Button
          variant="secondary"
          icon="paperclip"
          disabled={carregando}
          title="Lê o arquivo da chave direto para o cofre, sem área de transferência."
          data-acao="carregar-chave"
          onclick={() => carregarChave(c.id)}>Carregar de arquivo</Button
        >
      </div>
      <span class="ajuda" class:erro={erroDe(c.id) || aviso} role={aviso ? 'alert' : undefined}>
        {erroDe(c.id) ||
          aviso ||
          'Cole a chave ou use Carregar de arquivo. Depois de salvo, ela só aparece com revelar.'}
      </span>
    </div>
  {:else if c.tipo === 'segredo'}
    {@render campoSecreto(c)}
  {:else if c.tipo === 'totp'}
    <TotpCode
      id="form-codigo"
      code={totp?.codigo ?? null}
      remaining={totp?.restante ?? 0}
      copied={copiado === 'codigo'}
      oncopy={copiarTotp}
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
  <span class="avatar"><Icon name={entryType(tipo).icon} size="lg" /></span>
  <div class="identificacao">
    <h2 class="nome">{editando ? detalhe.nome : 'Nova credencial'}</h2>
    <div class="selos">
      <TypeBadge kind={tipo} />
      {#if editando}
        <EnvironmentBadge environment={detalhe.ambiente} />
        <span class="subtitulo">grupo {detalhe.grupo} · editando</span>
      {:else}
        <span class="subtitulo">preencha nome, grupo e ambiente</span>
      {/if}
    </div>
  </div>
  {#if editando && tipo === 'servidor'}
    <Button
      variant="secondary"
      icon={conectado ? 'check' : 'type-server'}
      disabled={conectando}
      title="Abre o seu terminal já conectado, sem PIN (token de uso único)."
      data-conectado={conectado ? 'sim' : undefined}
      onclick={conectar}>Conectar</Button
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
    <Field
      id="form-nome"
      label="Nome"
      bind:value={nome}
      placeholder={def.exemplo ?? 'Ex.: acme-wp-dev'}
      error={erroDe('nome')}
    />
    <Select
      id="form-tipo"
      label="Tipo"
      value={tipo}
      options={[{ value: tipo, label: entryType(tipo).label }]}
      disabled={editando}
    />
    <GroupCombobox id="form-grupo" bind:value={grupo} groups={grupos} error={erroDe('grupo')} />
    <div class="com-erro-select">
      <Select id="form-ambiente" label="Ambiente" bind:value={ambiente} options={AMBIENTES_FORM} />
      {#if erroDe('ambiente')}<span class="erro">{erroDe('ambiente')}</span>{/if}
    </div>
    <Field
      id="form-projeto"
      label="Projeto"
      bind:value={projeto}
      placeholder="Opcional"
      error={erroDe('projeto')}
    />
    <Field
      id="form-tags"
      label="Tags"
      bind:value={tags}
      placeholder="Separe por vírgula"
      error={erroDe('tags')}
    />
  </div>

  {#if def.linhas.length > 0}
    <section class="secao" aria-labelledby="secao-acesso">
      <SectionTitle id="secao-acesso" title={def.tituloAcesso ?? 'Acesso'} description={textoAcesso} />
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
    <SectionTitle
      id="secao-extras"
      title="Campos extras"
      description={tipo === 'generico' && editando
        ? 'Só eles são entregues, como WARD_<NOME>.'
        : (TEXTOS_EXTRAS[tipo] ?? TEXTOS_EXTRAS.padrao)}
    >
      {#snippet action()}
        <Button variant="ghost" icon="plus" onclick={adicionarExtra}>Adicionar campo</Button>
      {/snippet}
    </SectionTitle>
    {#if extras.length === 0}
      <p class="caixa-vazia" class:erro={erroDe('extras')}>
        {erroDe('extras') || 'Nenhum campo extra. Cada um tem nome, valor e a chave segredo.'}
      </p>
    {:else}
      {#each extras as x, i (x.chave)}
        <ExtraField
          id="extra-{x.chave}"
          bind:name={x.nome}
          bind:value={x.valor}
          bind:secret={x.segredo}
          stored={x.guardado}
          error={erros[`extras-${i + 1}`] ?? ''}
          onremove={() => extras.splice(i, 1)}
          onchange={() => {
            x.guardado = false;
            x.valor = '';
          }}
        />
      {/each}
      {#if erroDe('extras')}<span class="erro">{erroDe('extras')}</span>{/if}
    {/if}
  </section>

  <section class="secao" aria-labelledby="secao-anexos">
    <SectionTitle
      id="secao-anexos"
      title="Anexos"
      description={TEXTOS_ANEXOS[tipo] ?? TEXTOS_ANEXOS.padrao}
    >
      {#snippet action()}
        <Button variant="ghost" icon="paperclip" disabled={escolhendo} onclick={adicionarAnexo}
          >Adicionar arquivo</Button
        >
      {/snippet}
    </SectionTitle>
    {#if anexos.length === 0 && guardados.length === 0}
      <p class="caixa-vazia" class:erro={erroDe('anexos')}>
        Nenhum anexo. Use Adicionar arquivo (até 256 KiB somados).
      </p>
    {:else}
      {#each guardados as a, i (a.nome)}
        <Attachment
          name={a.nome}
          detail="{tamanhoLegivel(a.tamanho)} · {variavelDoAnexo(a.nome)}"
          onremove={() => removerGuardado(i)}
        />
      {/each}
      {#each anexos as a, i (a.id)}
        <Attachment
          name={a.nome}
          detail="{tamanhoLegivel(a.tamanho)} · {variavelDoAnexo(a.nome)}"
          onremove={() => removerAnexo(i)}
        />
      {/each}
    {/if}
    {#if avisoAnexo}<span class="erro" role="alert" id="aviso-anexo">{avisoAnexo}</span>{/if}
    {#if erroDe('anexos')}<span class="erro" id="erro-anexos">{erroDe('anexos')}</span>{/if}
  </section>

  <div class="nivel">
    <Icon name={nivel.icone} />
    <div class="textos-nivel">
      <span class="rotulo-nivel">Nível de acesso</span>
      <span class="nome-nivel">{nivel.rotulo}</span>
      <span class="explicacao">
        {editando
          ? nivel.explicacao
          : 'Toda entrada nova começa assim. Mudar o nível pede o PIN.'}
      </span>
    </div>
    <Button variant="secondary" icon="lock" disabled>Mudar nível</Button>
  </div>
</form>

{#if geradorPara}
  <div class="camada-popover">
    <GeneratorPopover field={geradorPara} onuse={usarGerada} onclose={() => (geradorPara = '')} />
  </div>
{/if}

<footer class="rodape">
  <span class="metadados">{editando ? '' : 'Obrigatórios: nome, grupo e ambiente.'}</span>
  {#if erroGeral}<Toast kind="error" message={erroGeral} />{/if}
  <Button variant="secondary" onclick={oncancelar}>Cancelar</Button>
  <Button
    variant="primary"
    type="submit"
    form="formulario-credencial"
    disabled={salvando || vazio}
  >
    {editando ? 'Salvar alterações' : 'Salvar'}
  </Button>
</footer>

<style>
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
  .identificacao {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  .nome {
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
  }
  .selos {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  .subtitulo {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subtitulo,
  .metadados {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .grade,
  .linha.dupla {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-16);
  }
  .grade {
    row-gap: var(--space-20);
  }
  .grade > :global(*),
  .linha > :global(*) {
    min-width: 0;
  }
  .secao {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }
  .linha {
    display: flex;
    flex-direction: column;
  }
  .com-erro-select {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .com-erro-select > :global(.select) {
    min-width: 0;
  }
  .nota {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .acao-chave {
    display: flex;
  }
  .rotulo {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .texto-nota {
    min-height: var(--space-96);
    padding: var(--space-12) var(--space-16);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-field);
    color: var(--color-text-primary);
    font: inherit;
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    resize: vertical;
  }
  .texto-nota:focus-visible {
    outline: none;
    border-color: var(--color-border-focus);
  }
  .texto-nota.com-erro {
    border-color: var(--color-danger-border);
  }
  .texto-nota.secreto {
    -webkit-text-security: disc;
  }
  .texto-nota::placeholder {
    color: var(--color-text-secondary);
  }
  .ajuda,
  .erro {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .erro,
  .ajuda.erro {
    color: var(--color-danger-text);
  }
  .caixa-vazia {
    margin: 0;
    padding: var(--space-16);
    border-radius: var(--radius-item);
    border: var(--line-thin) dashed var(--color-border-default);
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    text-align: center;
  }
  .caixa-vazia.erro {
    border-color: var(--color-danger-border);
    color: var(--color-danger-text);
  }
  .nivel {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .textos-nivel {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
  }
  .rotulo-nivel,
  .explicacao {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .nome-nivel {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .explicacao {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .camada-popover {
    position: absolute;
    right: var(--space-24);
    bottom: var(--space-80);
    z-index: 3;
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
  }
</style>
