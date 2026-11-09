import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// S11: nada inline (a CSP não tem 'unsafe-inline'), nada em data: URI, nada remoto.
// `--mode teste` gera `dist-teste/` para o Chrome com a ponte de teste (D7); o build
// normal (`dist/`, embutido no Tauri) não contém o código da ponte.
//
// `semLinksDeErro`: as mensagens de erro do runtime do Svelte terminam com um link de
// documentação (`https://svelte.dev/e/<código>`). Nunca é buscado, mas deixaria uma URL
// remota no pacote (V5.2); vira só o código. O que sobra de `http://` são os namespaces
// XML (`http://www.w3.org/...`), que são nomes e não endereços (ver `dev/testes/app-dist.py`).
function semLinksDeErro() {
  return {
    name: 'ward-sem-links-de-erro',
    renderChunk(codigo) {
      return { code: codigo.replaceAll('https://svelte.dev/e/', 'svelte/e/'), map: null };
    },
  };
}

export default defineConfig(({ mode }) => ({
  plugins: [svelte(), semLinksDeErro()],
  base: '/',
  publicDir: false,
  build: {
    outDir: mode === 'teste' ? 'dist-teste' : 'dist',
    emptyOutDir: true,
    sourcemap: false,
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
    cssCodeSplit: false,
    target: 'es2022',
    reportCompressedSize: false,
  },
  server: {
    host: '127.0.0.1',
    strictPort: true,
  },
}));
