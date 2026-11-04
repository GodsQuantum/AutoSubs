import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    sveltekit({
      preprocess: vitePreprocess(),
      adapter: adapter({ fallback: 'index.html', precompress: true }),
      paths: { relative: false }
    })
  ],
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:3000',
      '/fonts': 'http://127.0.0.1:3000'
    }
  }
});
