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
  import Botao from '../lib/componentes/Botao.svelte';
  import CampoPin from '../lib/componentes/CampoPin.svelte';
  import CampoSenha from '../lib/componentes/CampoSenha.svelte';
  import Destaque from '../lib/componentes/Destaque.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import IndicadorDePassos from '../lib/componentes/IndicadorDePassos.svelte';
  import PainelDeMarca from '../lib/componentes/PainelDeMarca.svelte';
  import Toast from '../lib/componentes/Toast.svelte';
  import { chamar } from '../lib/ponte/index.js';
  import {
    MAX_DIGITOS_PIN,
    MENSAGENS,
    erroDaSenha,
    forcaDaSenha,
    mensagemDoRust,
    pinValido,
  } from './primeiro-uso.js';

  /** @type {{ oncriado: () => void }} */
  let { oncriado } = $props();

  const NOMES = { 1: 'Criar cofre', 2: 'Senha mestra', 3: 'PIN' };
  const ITENS = [
    {
      icone: 'cadeado',
      titulo: 'Cifrado em repouso',
      texto: 'Argon2id e XChaCha20-Poly1305. Sem a senha mestra, o arquivo não abre.',
    },
    {
      icone: 'tipo-servidor',
      titulo: 'Fica só nesta máquina',
      texto: 'Em ~/.local/share/ward/, sem nuvem e sem sincronizar.',
    },
    {
      icone: 'relogio',
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
      const r = await chamar('criar_cofre', {
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
  <PainelDeMarca />
  <main class="conteudo">
    {#if passo === 1}
      <div class="formulario largo">
        <IndicadorDePassos passo={1} nome={NOMES[1]} />
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
              <span class="avatar"><Icone nome={item.icone} tamanho="sm" tom="marca" /></span>
              <span class="textos">
                <span class="titulo-item">{item.titulo}</span>
                <span class="texto-item">{item.texto}</span>
              </span>
            </li>
          {/each}
        </ul>
        <Botao id="primeiro-uso-criar" onclick={() => irPara(2)}>
          Criar cofre
        </Botao>
      </div>
    {:else if passo === 2}
      <form class="formulario" onsubmit={continuar} novalidate>
        <IndicadorDePassos passo={2} nome={NOMES[2]} />
        <header class="cabecalho">
          <h1>Senha mestra</h1>
          <p class="descricao">
            É a única chave do cofre. Use uma frase longa que você não usa em nenhum outro
            lugar.
          </p>
        </header>
        <CampoSenha
          id="primeiro-uso-senha"
          rotulo="Senha mestra"
          placeholder="Digite a senha mestra"
          ajuda="Mínimo de 12 caracteres."
          bind:valor={senha}
          bind:entrada={campoSenha}
          erro={erros.senha}
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
        <CampoSenha
          id="primeiro-uso-confirmar"
          rotulo="Confirmar senha mestra"
          placeholder="Digite a senha mestra de novo"
          bind:valor={confirmacao}
          bind:entrada={campoConfirmacao}
          erro={erros.confirmacao}
          oninput={() => (erros.confirmacao = '')}
        />
        <Destaque tipo="aviso" icone="alerta">
          A senha mestra não tem recuperação: se você esquecer, o cofre não abre. Depois,
          imprima o kit de emergência em Configurações.
        </Destaque>
        <div class="acoes">
          <Botao variante="secundario" onclick={() => irPara(1)}>Voltar</Botao>
          <Botao tipo="submit">Continuar</Botao>
        </div>
      </form>
    {:else}
      <form class="formulario" onsubmit={concluir} novalidate>
        <IndicadorDePassos passo={3} nome={NOMES[3]} />
        <header class="cabecalho">
          <h1>PIN de aprovação</h1>
          <p class="descricao">
            Com o PIN você aprova usos sensíveis e mudanças de nível de acesso. Use de 4 a 6
            dígitos.
          </p>
        </header>
        <div class="pin" onbeforeinput={recusaNoPin('pin')}>
          <CampoPin
            id="primeiro-uso-pin"
            rotulo="PIN"
            ajuda="4 a 6 dígitos."
            bind:valor={pin}
            erro={erros.pin}
          />
        </div>
        <div class="pin" onbeforeinput={recusaNoPin('confirmacao_pin')}>
          <CampoPin
            id="primeiro-uso-confirmar-pin"
            rotulo="Confirmar PIN"
            ajuda="Digite o mesmo PIN de novo."
            bind:valor={confirmacaoPin}
            erro={erros.confirmacao_pin}
          />
        </div>
        <Destaque tipo="info" icone="cadeado">
          O PIN não destrava o cofre. Ele só aprova usos e mudanças de nível. Cinco erros
          seguidos travam o cofre.
        </Destaque>
        {#if falha}<Toast tipo="erro" mensagem={falha} />{/if}
        <div class="acoes">
          <Botao variante="secundario" desabilitado={enviando} onclick={() => irPara(2)}>
            Voltar
          </Botao>
          <Botao tipo="submit" desabilitado={enviando}>
            {enviando ? 'Criando o cofre…' : 'Concluir'}
          </Botao>
        </div>
      </form>
    {/if}
  </main>
</div>

<style>
  .janela {
    display: flex;
    gap: var(--espaco-16);
    height: 100%;
    padding: var(--espaco-16);
    background: var(--cor-fundo-padrao);
  }
  .conteudo {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: var(--espaco-48);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
    overflow-y: auto;
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-24);
    width: var(--largura-coluna-acesso);
    margin: var(--espaco-0);
  }
  .largo {
    gap: var(--espaco-32);
  }
  .cabecalho {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
  }
  h1 {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-xl);
    line-height: var(--altura-linha-xl);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .descricao {
    margin: var(--espaco-0);
    color: var(--cor-texto-secundario);
  }
  .cartao {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    margin: var(--espaco-0);
    padding: var(--espaco-20);
    list-style: none;
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .item {
    display: flex;
    gap: var(--espaco-12);
  }
  .avatar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--controle-sm);
    height: var(--controle-sm);
    border-radius: var(--raio-controle);
    background: var(--cor-primaria-sutil);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
  }
  .titulo-item {
    font-weight: var(--peso-medio);
  }
  .texto-item {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .largo > :global(.botao) {
    width: 100%;
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
  .acoes {
    display: flex;
    gap: var(--espaco-12);
  }
  .acoes :global(.botao) {
    flex: 1;
  }
</style>
