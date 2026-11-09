<script>
  // App: com o cofre destravado, a casca com a `Navegação lateral` e a tela escolhida;
  // fora disso, o esqueleto da Fase 29 com o estado do daemon. As telas do M0 chegam nas
  // Fases 30–39, cada uma a partir dos nodes de docs/mission/design-nodes.md.
  import { onMount } from 'svelte';
  import Botao from './lib/componentes/Botao.svelte';
  import Logo from './lib/componentes/Logo.svelte';
  import NavegacaoLateral from './lib/componentes/NavegacaoLateral.svelte';
  import Toast from './lib/componentes/Toast.svelte';
  import { chamar } from './lib/ponte/index.js';
  import Credenciais from './telas/Credenciais.svelte';
  import GeradorSenha from './telas/GeradorSenha.svelte';
  import PrimeiroUso from './telas/PrimeiroUso.svelte';

  const TEXTOS = {
    'sem-cofre': 'Nenhum cofre criado ainda.',
    travado: 'O cofre está travado.',
    destravado: 'O cofre está destravado.',
  };
  const TELAS = ['credenciais', 'gerador', 'configuracoes'];

  let mensagem = $state('Conectando ao daemon…');
  let falhou = $state(false);
  let destravado = $state(false);
  // Fase 30: sem cofre, o primeiro uso; ao criar, o daemon deixa o cofre destravado.
  let semCofre = $state(false);
  let tela = $state('credenciais');
  let Vitrine = $state(null);

  async function consultar() {
    falhou = false;
    mensagem = 'Conectando ao daemon…';
    try {
      const r = await chamar('estado_do_cofre');
      destravado = r?.estado === 'destravado';
      semCofre = r?.estado === 'sem-cofre';
      mensagem = TEXTOS[r?.estado] ?? 'Estado desconhecido.';
    } catch (e) {
      falhou = true;
      mensagem = e.mensagem;
    }
  }

  function aoCriar() {
    semCofre = false;
    tela = 'credenciais';
    destravado = true;
  }

  // O cofre travou durante o uso (Fase 35: 5º PIN errado, S9): a casca sai e o estado
  // do daemon é consultado de novo. A tela `Destravar / Padrão` chega na Fase 31.
  function travou() {
    destravado = false;
    consultar();
  }

  function navegar(nova) {
    if (TELAS.includes(nova)) tela = nova;
  }

  onMount(async () => {
    // Só no build de teste (some do pacote de produção): a URL de abertura da ponte pode
    // pedir uma tela (`&tela=gerador`) para o QA no Chrome, e a vitrine de componentes.
    if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
      const { telaPedida } = await import('./lib/ponte/ponte-teste.js');
      const pedida = telaPedida();
      if (pedida && TELAS.includes(pedida)) {
        tela = pedida;
        destravado = true;
        return;
      }
    }
    consultar();
    if (import.meta.env.MODE === 'teste') {
      Vitrine = (await import('./lib/teste/Vitrine.svelte')).default;
    }
  });
</script>

{#if destravado}
  <div class="casca">
    <NavegacaoLateral ativo={tela} onnavegar={navegar} />
    {#if tela === 'credenciais'}
      <Credenciais ontravado={travou} />
    {:else if tela === 'gerador'}
      <GeradorSenha />
    {:else}
      <main class="vazio"><p class="estado">Esta tela ainda não existe.</p></main>
    {/if}
  </div>
{:else if semCofre}
  <PrimeiroUso oncriado={aoCriar} />
{:else}
  <main class="janela">
    <section class="painel">
      <Logo />
      <h1>Cofre de credenciais</h1>
      {#if falhou}
        <Toast tipo="erro" {mensagem} />
        <Botao variante="secundario" onclick={consultar}>Tentar de novo</Botao>
      {:else}
        <p class="estado">{mensagem}</p>
      {/if}
    </section>
    {#if Vitrine}<Vitrine />{/if}
  </main>
{/if}

<style>
  .casca {
    display: flex;
    gap: var(--espaco-16);
    height: 100%;
    padding: var(--espaco-16);
    background: var(--cor-fundo-padrao);
  }
  .vazio {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
  }
  .janela {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--espaco-32);
    min-height: 100%;
    padding: var(--espaco-48) var(--espaco-24);
    background: var(--cor-fundo-padrao);
  }
  .painel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--espaco-20);
    width: var(--largura-coluna-acesso);
    padding: var(--espaco-48);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
  }
  h1 {
    margin: 0;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-xl);
    line-height: var(--altura-linha-xl);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .estado {
    margin: 0;
    color: var(--cor-texto-secundario);
  }
</style>
