<script>
  // `Painel de marca` (717): painel das telas de acesso (destravar, primeiro uso).
  // Raio painel, borda borda/sutil nos 4 lados, fundo superficie/padrao, padding
  // espaco/48, conteúdo distribuído (marca no topo, mensagem no meio, rodapé embaixo).
  // Marca = `Logo / Horizontal` Marca; mensagem em Título/Destaque, descrição em
  // Texto/Corpo secundário e as três garantias com `Ícone/check` (gap espaco/12).
  import Icone from './Icone.svelte';
  import Logo from './Logo.svelte';

  const GARANTIAS = [
    'Offline: nada sai desta máquina.',
    'Cifrado com Argon2id e XChaCha20-Poly1305.',
    'Uso sensível só com a sua aprovação e PIN.',
  ];
</script>

<aside class="painel" aria-label="Ward">
  <div class="marca"><Logo /></div>
  <div class="mensagem">
    <p class="titulo">Credenciais de trabalho, fora da conversa.</p>
    <p class="descricao">
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
    /* 560 no FigCraft (largura fixa da instância, sem variable própria); é o mesmo
       valor de largura/dialogo-largo. */
    flex: 0 0 var(--largura-dialogo-largo);
    padding: var(--espaco-48);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .marca {
    display: flex;
    align-items: center;
  }
  .mensagem {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
  }
  p {
    margin: var(--espaco-0);
  }
  .titulo {
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-3xl);
    line-height: var(--altura-linha-3xl);
    letter-spacing: var(--espacamento-letra-titulo);
    color: var(--cor-texto-primario);
  }
  .descricao {
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
    color: var(--cor-texto-primario);
  }
  .rodape {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
</style>
