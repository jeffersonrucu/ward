<script>
  // Esqueleto do app (Fase 29): mostra se o daemon responde. As telas do M0 chegam nas
  // Fases 30–39, cada uma a partir dos nodes de docs/mission/design-nodes.md.
  import { onMount } from 'svelte';
  import Botao from './lib/componentes/Botao.svelte';
  import Logo from './lib/componentes/Logo.svelte';
  import Toast from './lib/componentes/Toast.svelte';
  import { chamar } from './lib/ponte/index.js';

  const TEXTOS = {
    'sem-cofre': 'Nenhum cofre criado ainda.',
    travado: 'O cofre está travado.',
    destravado: 'O cofre está destravado.',
  };

  let mensagem = $state('Conectando ao daemon…');
  let falhou = $state(false);
  let Vitrine = $state(null);

  async function consultar() {
    falhou = false;
    mensagem = 'Conectando ao daemon…';
    try {
      const r = await chamar('estado_do_cofre');
      mensagem = TEXTOS[r?.estado] ?? 'Estado desconhecido.';
    } catch (e) {
      falhou = true;
      mensagem = e.mensagem;
    }
  }

  onMount(async () => {
    consultar();
    // Vitrine de componentes: só no build de teste (some do pacote de produção).
    if (import.meta.env.MODE === 'teste') {
      Vitrine = (await import('./lib/teste/Vitrine.svelte')).default;
    }
  });
</script>

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

<style>
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
