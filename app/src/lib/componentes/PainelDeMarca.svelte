<script>
  // `Painel de marca` (717): coluna esquerda do primeiro uso e do destravar. Padding
  // espaco/48, raio painel, borda borda/sutil, fundo superficie/padrao; `Logo /
  // Horizontal` em cima, mensagem (Título/Destaque, Texto/Corpo secundário e as três
  // garantias com `Ícone/check`, gap espaco/16 e espaco/12) e rodapé em Texto/Legenda.
  import Icone from './Icone.svelte';
  import Logo from './Logo.svelte';

  const GARANTIAS = [
    'Offline: nada sai desta máquina.',
    'Cifrado com Argon2id e XChaCha20-Poly1305.',
    'Uso sensível só com a sua aprovação e PIN.',
  ];
</script>

<aside class="painel">
  <Logo />
  <div class="mensagem">
    <p class="titulo">Credenciais de trabalho, fora da conversa.</p>
    <p class="texto">
      O Claude recebe o acesso, nunca a senha. Cada uso respeita o nível de acesso da entrada.
    </p>
    <ul class="garantias">
      {#each GARANTIAS as g (g)}
        <li><Icone nome="check" /><span>{g}</span></li>
      {/each}
    </ul>
  </div>
  <p class="rodape">Só credenciais de trabalho · Linux</p>
</aside>

<style>
  .painel {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    flex: none;
    /* 560 no FigCraft (sem variable ligada; o valor é o de largura/dialogo-largo). */
    width: var(--largura-dialogo-largo);
    padding: var(--espaco-48);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .mensagem {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
  }
  .titulo {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-3xl);
    line-height: var(--altura-linha-3xl);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .texto {
    margin: var(--espaco-0);
    color: var(--cor-texto-secundario);
  }
  .garantias {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
    margin: var(--espaco-0);
    padding: var(--espaco-8) var(--espaco-0);
    list-style: none;
  }
  .garantias li {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  .rodape {
    margin: var(--espaco-0);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
</style>
