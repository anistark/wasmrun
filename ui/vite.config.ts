import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import path from 'path'

export default defineConfig(() => {
  const template = process.env.VITE_TEMPLATE as 'app' | 'console' || 'console'
  
  return {
    plugins: [preact()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      }
    },
    build: {
      outDir: `../templates-temp/${template}`,
      emptyOutDir: true,
      rollupOptions: {
        input: `./src/${template}/index.html`,
        output: {
          inlineDynamicImports: true,
          entryFileNames: `${template}.js`,
          chunkFileNames: `${template}.js`,
          assetFileNames: '[name].[ext]'
        }
      }
    },
    server: {
      port: 3000
    }
  }
})