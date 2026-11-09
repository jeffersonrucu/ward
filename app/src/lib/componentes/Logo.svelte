<script>
  // `Logo / Horizontal` (set 3300): símbolo 40×40 (controle/md) + "ward" em
  // Título/Destaque, gap espaco/8. O símbolo é o SVG exportado do `Logo / Símbolo`
  // (docs/design/logo/ward-simbolo.svg), servido como arquivo (img-src 'self').
  import simbolo from './ward-simbolo.svg';
</script>

<span class="logo">
  <img class="simbolo" src={simbolo} alt="" />
  <span class="palavra">ward</span>
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  .simbolo {
    width: var(--controle-md);
    height: var(--controle-md);
  }
  .palavra {
    display: inline-flex;
    align-items: center;
    height: var(--controle-md);
    padding-bottom: var(--espaco-4);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-3xl);
    line-height: var(--altura-linha-3xl);
    letter-spacing: var(--espacamento-letra-titulo);
    color: var(--cor-texto-primario);
  }
</style>
