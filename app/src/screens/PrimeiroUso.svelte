<script>
  // Tela `Primeiro uso / Criar cofre` (3537), `/ Senha mestra` (3619) e `/ PIN` (3733).
  // Janela com o `Painel de marca` e o painel `conteúdo` (fundo/sutil, borda borda/sutil,
  // raio painel, padding espaco/48) com a coluna `formulário` centrada (largura
  // coluna-acesso; gap espaco/32 no passo 1, espaco/24 nos outros).
  //
  // A senha mestra e o PIN são digitados aqui e ficam só no estado deste componente,
  // até o `criar_cofre`: o lado Rust confere tudo de novo (tamanho, dígitos e as duas
  // confirmações) e o daemon grava só o hash do PIN (D30). Depois de enviar, os valores
  // são apagados daqui; ao sair da tela, o componente some com eles. Nada vai ao
  // armazenamento do navegador, à URL ou a estado global.
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import StepIndicator from '../lib/components/StepIndicator.svelte';
  import BrandPanel from '../lib/components/BrandPanel.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';
  import {
    MAX_DIGITOS_PIN,
    MENSAGENS,
    erroDaSenha,
    forcaDaSenha,
    mensagemDoRust,
    pinValido,
  } from './primeiro-uso.js';

  /** @type {{ oncreated: () => void }} */
  let { oncreated: oncriado } = $props();

  const NOMES = { 1: 'Criar cofre', 2: 'Senha mestra', 3: 'PIN' };
  const ITENS = [
    {
      icone: 'lock',
      titulo: 'Cifrado em repouso',
      texto: 'Argon2id e XChaCha20-Poly1305. Sem a senha mestra, o arquivo não abre.',
    },
    {
      icone: 'type-server',
      titulo: 'Fica só nesta máquina',
      texto: 'Em ~/.local/share/ward/, sem nuvem e sem sincronizar.',
    },
    {
      icone: 'clock',
      titulo: 'Trava sozinho',
      texto: 'Ao bloquear a tela, ao suspender o computador e depois de 4 h sem uso.',
    },
  ];

  let passo = $state(1);
  let senha = $state('');
  let confirmacao = $state('');
  let pin = $state('');
  let confirmacaoPin = $state('');
  let erros = $state({ senha: '', confirmacao: '', pin: '', confirmacao_pin: '' });
  let falha = $state('');
  let enviando = $state(false);

  let campoSenha = $state();
  let campoConfirmacao = $state();

  const forca = $derived(forcaDaSenha(senha));

  function limparErros() {
    erros = { senha: '', confirmacao: '', pin: '', confirmacao_pin: '' };
    falha = '';
  }

  function apagarValores() {
    senha = '';
    confirmacao = '';
    pin = '';
    confirmacaoPin = '';
  }

  onDestroy(apagarValores);

  async function focar(id) {
    await tick();
    document.getElementById(id)?.focus();
  }

  async function irPara(n) {
    limparErros();
    passo = n;
    await focar({ 1: 'primeiro-uso-criar', 2: 'primeiro-uso-senha', 3: 'primeiro-uso-pin' }[n]);
  }

  // Passo 2: Enter (ou "Continuar") avança. Com a senha válida e a confirmação vazia,
  // o foco desce para a confirmação sem erro.
  async function continuar(e) {
    e.preventDefault();
    limparErros();
    const curta = erroDaSenha(senha);
    if (curta) {
      erros.senha = curta;
      campoSenha?.focus();
      return;
    }
    if (!confirmacao) {
      campoConfirmacao?.focus();
      return;
    }
    if (confirmacao !== senha) {
      erros.confirmacao = MENSAGENS.senhaDiferente;
      campoConfirmacao?.focus();
      return;
    }
    await irPara(3);
  }

  // Letra, símbolo ou 7º dígito no PIN: a caixa não aceita (o `Campo PIN` só guarda
  // dígitos, até 6) e a recusa aparece no campo.
  function recusaNoPin(campo) {
    return (e) => {
      const texto = e.data ?? e.dataTransfer?.getData('text/plain') ?? '';
      if (!texto) return;
      const alvo = e.target;
      const valor = campo === 'pin' ? pin : confirmacaoPin;
      const semSelecao = alvo?.selectionStart === alvo?.selectionEnd;
      if (/\D/.test(texto)) {
        erros[campo] = MENSAGENS.pinComLetra;
      } else if (semSelecao && valor.length + texto.length > MAX_DIGITOS_PIN) {
        erros[campo] = MENSAGENS.pinLongo;
      } else {
        erros[campo] = '';
      }
    };
  }

  async function concluir(e) {
    e.preventDefault();
    if (enviando) return;
    limparErros();
    if (!pinValido(pin)) {
      erros.pin = MENSAGENS.pinInvalido;
      await focar('primeiro-uso-pin');
      return;
    }
    if (!confirmacaoPin) {
      await focar('primeiro-uso-confirmar-pin');
      return;
    }
    if (confirmacaoPin !== pin) {
      erros.confirmacao_pin = MENSAGENS.pinDiferente;
      await focar('primeiro-uso-confirmar-pin');
      return;
    }
    enviando = true;
    try {
      const r = await call('criar_cofre', {
        dados: { senha, confirmacao, pin, confirmacao_pin: confirmacaoPin },
      });
      if (r?.criado) {
        apagarValores();
        oncriado();
        return;
      }
      for (const erro of r?.erros ?? []) {
        if (erro.campo in erros) erros[erro.campo] = mensagemDoRust(erro.motivo);
      }
      if (erros.senha || erros.confirmacao) {
        const deSenha = { senha: erros.senha, confirmacao: erros.confirmacao };
        passo = 2;
        await tick();
        erros = { ...erros, ...deSenha };
        (erros.senha ? campoSenha : campoConfirmacao)?.focus();
      } else {
        await focar(erros.pin ? 'primeiro-uso-pin' : 'primeiro-uso-confirmar-pin');
      }
    } catch (err) {
      falha = mensagemDoRust(err?.mensagem) || 'Não foi possível criar o cofre.';
    } finally {
      enviando = false;
    }
  }

  focar('primeiro-uso-criar');
</script>

<div class="janela">
  <BrandPanel />
  <main class="conteudo">
    {#if passo === 1}
      <div class="formulario largo">
        <StepIndicator step={1} name={NOMES[1]} />
        <header class="cabecalho">
          <h1>Criar seu cofre</h1>
          <p class="descricao">
            O Ward guarda suas credenciais de trabalho num arquivo cifrado. Leva menos de um
            minuto.
          </p>
        </header>
        <ul class="cartao">
          {#each ITENS as item (item.titulo)}
            <li class="item">
              <span class="avatar"><Icon name={item.icone} size="sm" tone="brand" /></span>
              <span class="textos">
                <span class="titulo-item">{item.titulo}</span>
                <span class="texto-item">{item.texto}</span>
              </span>
            </li>
          {/each}
        </ul>
        <Button id="primeiro-uso-criar" onclick={() => irPara(2)}>
          Criar cofre
        </Button>
      </div>
    {:else if passo === 2}
      <form class="formulario" onsubmit={continuar} novalidate>
        <StepIndicator step={2} name={NOMES[2]} />
        <header class="cabecalho">
          <h1>Senha mestra</h1>
          <p class="descricao">
            É a única chave do cofre. Use uma frase longa que você não usa em nenhum outro
            lugar.
          </p>
        </header>
        <PasswordField
          id="primeiro-uso-senha"
          label="Senha mestra"
          placeholder="Digite a senha mestra"
          help="Mínimo de 12 caracteres."
          bind:value={senha}
          bind:input={campoSenha}
          error={erros.senha}
          oninput={() => (erros.senha = '')}
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
        <PasswordField
          id="primeiro-uso-confirmar"
          label="Confirmar senha mestra"
          placeholder="Digite a senha mestra de novo"
          bind:value={confirmacao}
          bind:input={campoConfirmacao}
          error={erros.confirmacao}
          oninput={() => (erros.confirmacao = '')}
        />
        <Highlight kind="warning" icon="alert">
          A senha mestra não tem recuperação: se você esquecer, o cofre não abre. Depois,
          imprima o kit de emergência em Configurações.
        </Highlight>
        <div class="acoes">
          <Button variant="secondary" onclick={() => irPara(1)}>Voltar</Button>
          <Button type="submit">Continuar</Button>
        </div>
      </form>
    {:else}
      <form class="formulario" onsubmit={concluir} novalidate>
        <StepIndicator step={3} name={NOMES[3]} />
        <header class="cabecalho">
          <h1>PIN de aprovação</h1>
          <p class="descricao">
            Com o PIN você aprova usos sensíveis e mudanças de nível de acesso. Use de 4 a 6
            dígitos.
          </p>
        </header>
        <div class="pin" onbeforeinput={recusaNoPin('pin')}>
          <PinField
            id="primeiro-uso-pin"
            label="PIN"
            help="4 a 6 dígitos."
            bind:value={pin}
            error={erros.pin}
          />
        </div>
        <div class="pin" onbeforeinput={recusaNoPin('confirmacao_pin')}>
          <PinField
            id="primeiro-uso-confirmar-pin"
            label="Confirmar PIN"
            help="Digite o mesmo PIN de novo."
            bind:value={confirmacaoPin}
            error={erros.confirmacao_pin}
          />
        </div>
        <Highlight kind="info" icon="lock">
          O PIN não destrava o cofre. Ele só aprova usos e mudanças de nível. Cinco erros
          seguidos travam o cofre.
        </Highlight>
        {#if falha}<Toast kind="error" message={falha} />{/if}
        <div class="acoes">
          <Button variant="secondary" disabled={enviando} onclick={() => irPara(2)}>
            Voltar
          </Button>
          <Button type="submit" disabled={enviando}>
            {enviando ? 'Criando o cofre…' : 'Concluir'}
          </Button>
        </div>
      </form>
    {/if}
  </main>
</div>

<style>
  .janela {
    display: flex;
    gap: var(--space-16);
    height: 100%;
    padding: var(--space-16);
    background: var(--color-bg-default);
  }
  .conteudo {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: var(--space-48);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow-y: auto;
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
    width: var(--width-column-access);
    margin: var(--space-0);
  }
  .largo {
    gap: var(--space-32);
  }
  .cabecalho {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .descricao {
    margin: var(--space-0);
    color: var(--color-text-secondary);
  }
  .cartao {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    margin: var(--space-0);
    padding: var(--space-20);
    list-style: none;
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .item {
    display: flex;
    gap: var(--space-12);
  }
  .avatar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border-radius: var(--radius-control);
    background: var(--color-primary-subtle);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .titulo-item {
    font-weight: var(--weight-medium);
  }
  .texto-item {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .largo > :global(.button) {
    width: 100%;
  }
  .forca {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .barras {
    display: flex;
    gap: var(--space-6);
  }
  .barra {
    flex: 1;
    height: var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--color-border-default);
  }
  .barra[data-tom='perigo'] {
    background: var(--color-danger-default);
  }
  .barra[data-tom='aviso'] {
    background: var(--color-warning-default);
  }
  .barra[data-tom='sucesso'] {
    background: var(--color-success-default);
  }
  .legenda {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .legenda[data-tom='perigo'] {
    color: var(--color-danger-text);
  }
  .legenda[data-tom='aviso'] {
    color: var(--color-warning-text);
  }
  .legenda[data-tom='sucesso'] {
    color: var(--color-success-text);
  }
  .acoes {
    display: flex;
    gap: var(--space-12);
  }
  .acoes :global(.button) {
    flex: 1;
  }
</style>
