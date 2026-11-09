<script>
  // "Restaurar backup" de `Configurações` (Fase 47, V7.7; D17, D39, S9). Frames
  // `Restaurar backup / Versões` (16349, diálogo 16458) e `/ Confirmar` (16512, diálogo
  // 16621), o `Diálogo de restauração` (set 13033): largura largura/dialogo-largo, `hug`,
  // raio janela, padding espaco/24, gap espaco/20, fundo superficie/elevada, borda
  // borda/padrao, Sombra/Modal. `cabeçalho` gap espaco/8: `textos` (gap espaco/4: título
  // Título/Modal, subtítulo Texto/Pequeno secundário) e `Ícone/fechar`.
  // - Versões: `versões` gap espaco/8 com os `Item de versão` (11264), a legenda
  //   (Texto/Legenda secundário) e o rodapé à direita (gap espaco/12): Cancelar e
  //   Continuar.
  // - Confirmar: `conteúdo` gap espaco/16 com o item escolhido, o `Campo senha` (17502)
  //   "Senha mestra desta versão", o `Campo PIN` (650) "PIN atual" (o frame não tem; o
  //   daemon exige o PIN de agora, Fase 45) e o `Destaque` aviso (18684); a legenda e o
  //   rodapé com Voltar à esquerda e "Restaurar esta versão" à direita.
  //
  // As versões vêm do daemon só com o nome, o instante e duas marcas (sem tamanho,
  // origem, geração nem conteúdo): a linha de baixo é "Cópia local" (Fase 49a: "de outra
  // senha mestra", ou em aviso "gravação mais antiga do que a data diz" para a cópia
  // antiga com nome novo). A senha da versão e o PIN ficam
  // só no estado local deste componente, saem dele antes da resposta do pedido e ao
  // fechar ou voltar. Nada vai para armazenamento, URL ou log.
  import { onDestroy, tick } from 'svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import CampoPin from '../lib/componentes/CampoPin.svelte';
  import CampoSenha from '../lib/componentes/CampoSenha.svelte';
  import Destaque from '../lib/componentes/Destaque.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import ItemDeVersao from '../lib/componentes/ItemDeVersao.svelte';
  import { chamar } from '../lib/ponte/index.js';
  import { mensagemDoRust } from './primeiro-uso.js';
  import {
    detalheDaVersao,
    avisoDaTroca,
    dataDaVersao,
    diaDaVersao,
    legendaDasVersoes,
    quandoFoi,
  } from './restaurar.js';

  /**
   * @type {{ aberto?: boolean, onfechar?: () => void,
   *   onrestaurado?: (r: { instante: number, entradas: number }) => void,
   *   ontravado?: () => void }}
   */
  let { aberto = false, onfechar = undefined, onrestaurado = undefined, ontravado = undefined } =
    $props();

  const ID_SENHA = 'restaurar-senha';
  const ID_PIN = 'restaurar-pin';

  let dialogo = $state();
  /** 'versoes' | 'confirmar' */
  let etapa = $state('versoes');
  let versoes = $state([]);
  let escolhida = $state('');
  let carregando = $state(false);
  let senha = $state('');
  let pin = $state('');
  let erroSenha = $state('');
  let erroPin = $state('');
  let falha = $state('');
  let enviando = $state(false);
  let agora = $state(new Date());

  const versao = $derived(versoes.find((v) => v.nome === escolhida) ?? null);

  function apagar() {
    senha = '';
    pin = '';
  }

  function reiniciar() {
    apagar();
    etapa = 'versoes';
    escolhida = '';
    versoes = [];
    erroSenha = '';
    erroPin = '';
    falha = '';
    enviando = false;
  }

  async function carregar() {
    carregando = true;
    falha = '';
    agora = new Date();
    try {
      const lista = await chamar('listar_backups');
      versoes = Array.isArray(lista) ? lista : [];
      if (!versoes.some((v) => v.nome === escolhida)) escolhida = '';
    } catch (e) {
      versoes = [];
      escolhida = '';
      if (e?.codigo === 'travado') {
        ontravado?.();
        return;
      }
      falha = mensagemDoRust(e?.mensagem) || 'Não foi possível ler as versões do backup.';
    } finally {
      carregando = false;
    }
  }

  $effect(() => {
    if (!dialogo) return;
    if (aberto && !dialogo.open) {
      reiniciar();
      dialogo.showModal();
      carregar();
    }
    if (!aberto && dialogo.open) dialogo.close();
  });

  onDestroy(apagar);

  function fechar() {
    apagar();
    onfechar?.();
  }

  async function focar(id) {
    await tick();
    document.getElementById(id)?.focus();
  }

  function continuar() {
    if (!versao) return;
    apagar();
    erroSenha = '';
    erroPin = '';
    falha = '';
    etapa = 'confirmar';
    focar(ID_SENHA);
  }

  function voltar() {
    apagar();
    erroSenha = '';
    erroPin = '';
    falha = '';
    etapa = 'versoes';
  }

  async function restaurar(e) {
    e?.preventDefault();
    if (enviando || !versao) return;
    erroSenha = '';
    erroPin = '';
    falha = '';
    if (!senha) {
      erroSenha = 'Digite a senha mestra desta versão.';
      return focar(ID_SENHA);
    }
    if (pin.length < 4) {
      erroPin = 'O PIN tem de 4 a 6 dígitos.';
      return focar(ID_PIN);
    }
    // A senha e o PIN saem do estado antes da resposta: a tela não os guarda nem no erro.
    const escolhidaAgora = versao;
    const args = { dados: { versao: escolhidaAgora.nome, senha, pin } };
    apagar();
    enviando = true;
    try {
      const r = await chamar('restaurar_backup', args);
      args.dados = null;
      enviando = false;
      onrestaurado?.({ instante: escolhidaAgora.instante, entradas: r?.entradas ?? 0 });
      return;
    } catch (err) {
      args.dados = null;
      enviando = false;
      const codigo = err?.codigo ?? '';
      if (codigo === 'senha-incorreta') {
        erroSenha = 'Essa senha não abre esta versão. Nada mudou no cofre atual.';
        return focar(ID_SENHA);
      }
      if (codigo === 'pin-incorreto') {
        erroPin = 'PIN incorreto. Nada foi alterado.';
        if (await travou()) return;
        return focar(ID_PIN);
      }
      if (codigo === 'travado' || codigo === 'cofre-mudou') {
        ontravado?.();
        return;
      }
      if (codigo === 'backup-inexistente') {
        // A versão saiu da pasta (rotação ou outro processo): a lista volta atualizada.
        etapa = 'versoes';
        await carregar();
        falha = mensagemDoRust(err?.mensagem);
        return;
      }
      falha = mensagemDoRust(err?.mensagem) || 'Não foi possível restaurar esta versão.';
      await travou();
    }
  }

  /** Depois de um erro: o 5.º PIN errado (ou uma falha ao gravar) trava o cofre no daemon. */
  async function travou() {
    try {
      const r = await chamar('estado_do_cofre');
      if (r?.estado === 'destravado') return false;
    } catch {
      // Sem resposta, trata como travado: a tela de destravar consulta de novo.
    }
    ontravado?.();
    return true;
  }
</script>

<dialog
  bind:this={dialogo}
  class="dialogo"
  aria-labelledby="restaurar-titulo"
  aria-describedby="restaurar-subtitulo"
  data-teste="restaurar-backup"
  data-etapa={etapa}
  onclose={fechar}
>
  <div class="cabecalho">
    <div class="textos">
      <h2 id="restaurar-titulo">{etapa === 'versoes' ? 'Restaurar backup' : 'Confirmar restauração'}</h2>
      <p id="restaurar-subtitulo" class="subtitulo">
        {#if etapa === 'versoes'}
          Escolha a versão que volta a ser o cofre. O cofre atual é guardado como backup antes da
          troca.
        {:else}
          Digite a senha mestra que valia nesta versão. Ela pode ser diferente da atual.
        {/if}
      </p>
    </div>
    <button class="fechar" type="button" aria-label="Fechar" onclick={fechar}>
      <Icone nome="fechar" />
    </button>
  </div>

  {#if etapa === 'versoes'}
    {#if carregando && versoes.length === 0}
      <p class="vazio" role="status">Procurando versões…</p>
    {:else if versoes.length === 0 && !falha}
      <p class="vazio" role="status">
        Ainda não há versões. O Ward guarda uma a cada mudança no cofre.
      </p>
    {:else if versoes.length > 0}
      <div class="versoes" role="radiogroup" aria-label="Versões do backup">
        {#each versoes as v, i (v.nome)}
          <ItemDeVersao
            id="restaurar-versao-{i}"
            valor={v.nome}
            data={dataDaVersao(v.instante)}
            detalhe={detalheDaVersao(v)}
            alerta={v.fora_de_ordem === true}
            quando={quandoFoi(v.instante, agora)}
            marcado={escolhida === v.nome}
            onescolher={(n) => (escolhida = n)}
          />
        {/each}
      </div>
      <p class="legenda">{legendaDasVersoes(versoes.length)}</p>
    {/if}
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <div class="rodape">
      <Botao variante="secundario" onclick={fechar}>Cancelar</Botao>
      <Botao variante="primario" desabilitado={!versao} onclick={continuar}>Continuar</Botao>
    </div>
  {:else if versao}
    <form class="conteudo" onsubmit={restaurar} novalidate>
      <ItemDeVersao
        id="restaurar-escolhida"
        estatico
        data={dataDaVersao(versao.instante)}
        detalhe={detalheDaVersao(versao)}
        alerta={versao.fora_de_ordem === true}
        quando={quandoFoi(versao.instante, agora)}
      />
      <CampoSenha
        rotulo="Senha mestra desta versão"
        id={ID_SENHA}
        bind:valor={senha}
        ajuda="É a senha que valia em {diaDaVersao(versao.instante)}."
        erro={erroSenha}
      />
      <CampoPin
        rotulo="PIN atual"
        id={ID_PIN}
        bind:valor={pin}
        ajuda="O PIN de agora confirma a troca. 5 erros seguidos travam o cofre."
        erro={erroPin}
      />
      <Destaque tipo="aviso" icone="alerta">{avisoDaTroca(versao.instante)}</Destaque>
      <button class="visualmente-oculto" type="submit" tabindex="-1">Restaurar esta versão</button>
    </form>
    <p class="legenda">Se a senha não abrir esta versão, nada muda no cofre atual.</p>
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <div class="rodape separado">
      <Botao variante="secundario" onclick={voltar}>Voltar</Botao>
      <Botao variante="primario" desabilitado={enviando} onclick={restaurar}
        >Restaurar esta versão</Botao
      >
    </div>
  {/if}
</dialog>

<style>
  .dialogo {
    width: var(--largura-dialogo-largo);
    max-width: calc(100vw - 2 * var(--espaco-24));
    padding: var(--espaco-24);
    border-radius: var(--raio-janela);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    color: var(--cor-texto-primario);
    box-shadow: var(--sombra-modal);
  }
  .dialogo[open] {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
  }
  .dialogo::backdrop {
    background: var(--cor-fundo-sobreposicao);
  }
  .cabecalho {
    display: flex;
    align-items: flex-start;
    gap: var(--espaco-8);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  p {
    margin: var(--espaco-0);
  }
  .subtitulo,
  .vazio {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .fechar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icone-md);
    height: var(--icone-md);
    padding: var(--espaco-0);
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    cursor: pointer;
  }
  .fechar:focus-visible {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
  /* Cinco itens à vista, como no frame (74 px cada: padding, data, gap, detalhe e
     borda); as mais antigas rolam. */
  .versoes {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
    max-height: calc(
      5 *
        (
          2 * var(--espaco-16) + var(--altura-linha-base) + var(--espaco-2) +
            var(--altura-linha-sm) + 2 * var(--linha-fina)
        ) +
        4 * var(--espaco-8)
    );
    overflow-y: auto;
  }
  .conteudo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    margin: var(--espaco-0);
  }
  .legenda {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .falha {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-perigo-texto);
  }
  .rodape {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--espaco-12);
  }
  .rodape.separado {
    justify-content: space-between;
  }
</style>
