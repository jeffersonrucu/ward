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
  import Button from '../lib/components/Button.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import VersionItem from '../lib/components/VersionItem.svelte';
  import { call } from '../lib/bridge/index.js';
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
      const lista = await call('listar_backups');
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
      const r = await call('restaurar_backup', args);
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
      const r = await call('estado_do_cofre');
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
      <Icon name="close" />
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
          <VersionItem
            id="restaurar-versao-{i}"
            value={v.nome}
            date={dataDaVersao(v.instante)}
            detail={detalheDaVersao(v)}
            alert={v.fora_de_ordem === true}
            when={quandoFoi(v.instante, agora)}
            checked={escolhida === v.nome}
            onchoose={(n) => (escolhida = n)}
          />
        {/each}
      </div>
      <p class="legenda">{legendaDasVersoes(versoes.length)}</p>
    {/if}
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <div class="rodape">
      <Button variant="secondary" onclick={fechar}>Cancelar</Button>
      <Button variant="primary" disabled={!versao} onclick={continuar}>Continuar</Button>
    </div>
  {:else if versao}
    <form class="conteudo" onsubmit={restaurar} novalidate>
      <VersionItem
        id="restaurar-escolhida"
        staticView
        date={dataDaVersao(versao.instante)}
        detail={detalheDaVersao(versao)}
        alert={versao.fora_de_ordem === true}
        when={quandoFoi(versao.instante, agora)}
      />
      <PasswordField
        label="Senha mestra desta versão"
        id={ID_SENHA}
        bind:value={senha}
        help="É a senha que valia em {diaDaVersao(versao.instante)}."
        error={erroSenha}
      />
      <PinField
        label="PIN atual"
        id={ID_PIN}
        bind:value={pin}
        help="O PIN de agora confirma a troca. 5 erros seguidos travam o cofre."
        error={erroPin}
      />
      <Highlight kind="warning" icon="alert">{avisoDaTroca(versao.instante)}</Highlight>
      <button class="visually-hidden" type="submit" tabindex="-1">Restaurar esta versão</button>
    </form>
    <p class="legenda">Se a senha não abrir esta versão, nada muda no cofre atual.</p>
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <div class="rodape separado">
      <Button variant="secondary" onclick={voltar}>Voltar</Button>
      <Button variant="primary" disabled={enviando} onclick={restaurar}
        >Restaurar esta versão</Button
      >
    </div>
  {/if}
</dialog>

<style>
  .dialogo {
    width: var(--width-wide-dialog);
    max-width: calc(100vw - 2 * var(--space-24));
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-modal);
  }
  .dialogo[open] {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
  }
  .dialogo::backdrop {
    background: var(--color-bg-overlay);
  }
  .cabecalho {
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  p {
    margin: var(--space-0);
  }
  .subtitulo,
  .vazio {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .fechar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--icon-md);
    height: var(--icon-md);
    padding: var(--space-0);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
  }
  .fechar:focus-visible {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
  /* Cinco itens à vista, como no frame (74 px cada: padding, data, gap, detalhe e
     borda); as mais antigas rolam. */
  .versoes {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    max-height: calc(
      5 *
        (
          2 * var(--space-16) + var(--line-height-base) + var(--space-2) +
            var(--line-height-sm) + 2 * var(--line-thin)
        ) +
        4 * var(--space-8)
    );
    overflow-y: auto;
  }
  .conteudo {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    margin: var(--space-0);
  }
  .legenda {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .falha {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-danger-text);
  }
  .rodape {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-12);
  }
  .rodape.separado {
    justify-content: space-between;
  }
</style>
