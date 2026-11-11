import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// S11: nothing inline (the CSP has no 'unsafe-inline'), nothing in a data: URI, nothing
// remote. The test mode (`npm run build:test`) writes the test output folder for Chrome
// with the test bridge (D7); the normal build (`dist/`, embedded in Tauri) does not
// contain the bridge code.
//
// `noErrorLinks`: the error messages of the Svelte runtime end with a documentation link
// (`https://svelte.dev/e/<code>`). It is never fetched, but it would leave a remote URL
// in the package (V5.2); it becomes just the code. What is left of `http://` are the XML
// namespaces (`http://www.w3.org/...`), which are names and not addresses (see
// `dev/tests/app-dist.py`).
function noErrorLinks() {
  return {
    name: 'ward-no-error-links',
    renderChunk(chunk) {
      return { code: chunk.replaceAll('https://svelte.dev/e/', 'svelte/e/'), map: null };
    },
  };
}

export default defineConfig(({ mode }) => ({
  plugins: [svelte(), noErrorLinks()],
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
