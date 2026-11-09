<script>
  // Cartão `pendrive` de `Configurações / Backup externo ligado` (FigCraft 16318, Fase
  // 48): superfície/padrão, borda borda/foco (6:71), raio item, padding espaco/20, gap
  // espaco/16. Avatar 40 (controle/md) superfície/elevada com borda/padrão e raio
  // controle; textos com gap espaco/2: nome Texto/Corpo forte, detalhe Texto/Pequeno
  // secundário, estado Texto/Legenda em sucesso/texto (6:86). `Botão` secundário
  // "Trocar" à direita.
  //
  // O rótulo vem do udisks2 (não confiável): chega do lado Rust com o invisível em
  // escape (D51) e aqui é só texto (interpolação do Svelte, nunca HTML), isolado na
  // direção (`<bdi>`). O design mostra o caminho de montagem; o protocolo não o manda
  // à interface (Fase 46), então o detalhe diz só se está conectado e montado.
  import Botao from './Botao.svelte';
  import Icone from './Icone.svelte';

  /**
   * @type {{ nome: string, uuid: string, situacao: string, estado: string,
   *   montado?: boolean, ontrocar?: () => void, desabilitado?: boolean }}
   */
  let {
    nome,
    uuid,
    situacao,
    estado,
    montado = false,
    ontrocar = undefined,
    desabilitado = false,
  } = $props();
</script>

<div class="cartao" id="config-pendrive">
  <span class="avatar" aria-hidden="true">
    <Icone nome={montado ? 'sucesso' : 'relogio'} tom={montado ? 'sucesso' : undefined} />
  </span>
  <span class="textos">
    <span class="nome"><bdi>{nome}</bdi></span>
    <span class="detalhe">UUID <bdi class="uuid">{uuid}</bdi> · {situacao}</span>
    <span class="estado" class:montado>{estado}</span>
  </span>
  <Botao variante="secundario" onclick={() => ontrocar?.()} {desabilitado} aria-label="Trocar pendrive"
    >Trocar</Botao
  >
</div>

<style>
  .cartao {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-20);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-foco);
    background: var(--cor-superficie-padrao);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--controle-md);
    height: var(--controle-md);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
    min-width: 0;
  }
  .nome,
  .detalhe,
  .estado {
    overflow-wrap: anywhere;
  }
  bdi {
    unicode-bidi: isolate;
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .detalhe {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .uuid {
    direction: ltr;
  }
  .estado {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .estado.montado {
    color: var(--cor-sucesso-texto);
  }
</style>
