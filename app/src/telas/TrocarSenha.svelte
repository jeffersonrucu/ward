<script>
  // "Trocar" da senha mestra em `Configurações / Geral` (Fase 38, V5.22). Sem frame
  // próprio no FigCraft: monta o `Modal` (611) com três `Campo senha` (17511), o
  // medidor de força do primeiro uso e o `Botão` (263).
  //
  // Pede a senha atual (o daemon a confere: D52) e a nova duas vezes, com a regra do
  // primeiro uso (12 caracteres ou mais; o Rust confere de novo). Os três valores ficam
  // só no estado local deste componente; saem do estado antes da resposta do pedido e
  // ao fechar. Nada vai para armazenamento, URL ou log.
  import { onDestroy, tick } from 'svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import CampoSenha from '../lib/componentes/CampoSenha.svelte';
  import Modal from '../lib/componentes/Modal.svelte';
  import { chamar } from '../lib/ponte/index.js';
  import { MENSAGENS, erroDaSenha, forcaDaSenha, mensagemDoRust } from './primeiro-uso.js';

  /**
   * @type {{ aberto?: boolean, onfechar?: () => void, ontrocada?: () => void,
   *   onfalha?: (codigo: string) => void }}
   */
  let { aberto = false, onfechar = undefined, ontrocada = undefined, onfalha = undefined } = $props();

  let atual = $state('');
  let nova = $state('');
  let confirmacao = $state('');
  let erros = $state({ atual: '', nova: '', confirmacao: '' });
  let falha = $state('');
  let enviando = $state(false);
  let campoAtual = $state();
  let campoNova = $state();
  let campoConfirmacao = $state();

  const forca = $derived(forcaDaSenha(nova));

  function apagar() {
    atual = '';
    nova = '';
    confirmacao = '';
  }

  function limparErros() {
    erros = { atual: '', nova: '', confirmacao: '' };
    falha = '';
  }

  $effect(() => {
    if (aberto) {
      apagar();
      limparErros();
      tick().then(() => campoAtual?.focus());
    }
  });

  onDestroy(apagar);

  function fechar() {
    apagar();
    limparErros();
    onfechar?.();
  }

  async function enviar(e) {
    e?.preventDefault();
    if (enviando) return;
    limparErros();
    if (!atual) {
      erros.atual = 'Digite a senha mestra atual.';
      campoAtual?.focus();
      return;
    }
    const curta = erroDaSenha(nova);
    if (curta) {
      erros.nova = curta;
      campoNova?.focus();
      return;
    }
    if (confirmacao !== nova) {
      erros.confirmacao = MENSAGENS.senhaDiferente;
      campoConfirmacao?.focus();
      return;
    }
    // Os valores saem do estado antes da resposta: a tela não os guarda nem no erro.
    const args = { dados: { atual, nova, confirmacao } };
    apagar();
    enviando = true;
    try {
      const r = await chamar('trocar_senha_mestra', args);
      if (r?.trocado) {
        ontrocada?.();
        return;
      }
      for (const erro of r?.erros ?? []) {
        if (erro.campo in erros) erros[erro.campo] = mensagemDoRust(erro.motivo);
      }
    } catch (err) {
      if (err?.codigo === 'senha-incorreta') {
        erros.atual = 'Senha mestra incorreta. Nada foi alterado.';
      } else {
        falha = mensagemDoRust(err?.mensagem) || 'Não foi possível trocar a senha mestra.';
        onfalha?.(err?.codigo ?? '');
      }
    } finally {
      args.dados = null;
      enviando = false;
    }
    await tick();
    campoAtual?.focus();
  }
</script>

<Modal titulo="Trocar senha mestra" {aberto} onfechar={fechar}>
  <form class="corpo" onsubmit={enviar} novalidate data-teste="trocar-senha">
    <p class="explicacao">
      A senha atual confirma a troca. Depois dela, o cofre só abre com a nova; os backups
      antigos continuam com a senha que valia quando foram gravados.
    </p>
    <CampoSenha
      rotulo="Senha mestra atual"
      id="trocar-senha-atual"
      bind:valor={atual}
      bind:entrada={campoAtual}
      autocomplete="current-password"
      erro={erros.atual}
    />
    <CampoSenha
      rotulo="Nova senha mestra"
      id="trocar-senha-nova"
      bind:valor={nova}
      bind:entrada={campoNova}
      ajuda="Mínimo de 12 caracteres."
      erro={erros.nova}
    />
    <div class="forca" aria-live="polite">
      <div class="barras" aria-hidden="true">
        {#each [1, 2, 3, 4] as n (n)}
          <span class="barra" data-tom={forca && n <= forca.segmentos ? forca.tom : undefined}
          ></span>
        {/each}
      </div>
      <span class="legenda" data-tom={forca?.tom}>
        {forca ? `Força: ${forca.rotulo}` : 'Força: digite a senha'}
      </span>
    </div>
    <CampoSenha
      rotulo="Confirmar nova senha"
      id="trocar-senha-confirmacao"
      bind:valor={confirmacao}
      bind:entrada={campoConfirmacao}
      erro={erros.confirmacao}
    />
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <button class="visualmente-oculto" type="submit" tabindex="-1">Trocar senha</button>
  </form>
  {#snippet rodape()}
    <Botao variante="secundario" onclick={fechar}>Cancelar</Botao>
    <Botao variante="primario" desabilitado={enviando} onclick={enviar}>Trocar senha</Botao>
  {/snippet}
</Modal>

<style>
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    margin: var(--espaco-0);
    color: var(--cor-texto-primario);
  }
  p {
    margin: var(--espaco-0);
  }
  .explicacao {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .falha {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-perigo-texto);
  }
  .forca {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
  .barras {
    display: flex;
    gap: var(--espaco-6);
  }
  .barra {
    flex: 1;
    height: var(--espaco-4);
    border-radius: var(--raio-pilula);
    background: var(--cor-borda-padrao);
  }
  .barra[data-tom='perigo'] {
    background: var(--cor-perigo-padrao);
  }
  .barra[data-tom='aviso'] {
    background: var(--cor-aviso-padrao);
  }
  .barra[data-tom='sucesso'] {
    background: var(--cor-sucesso-padrao);
  }
  .legenda {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .legenda[data-tom='perigo'] {
    color: var(--cor-perigo-texto);
  }
  .legenda[data-tom='aviso'] {
    color: var(--cor-aviso-texto);
  }
  .legenda[data-tom='sucesso'] {
    color: var(--cor-sucesso-texto);
  }
</style>
