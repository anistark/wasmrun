/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{js,ts,jsx,tsx}', './src/**/*.html'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // Dark theme colors matching current design
        dark: {
          bg: '#121212',
          surface: '#1e1e2e',
          surface2: '#252b37',
          surface3: '#313244',
          text: '#ffffff',
          textMuted: '#cdd6f4',
          textDim: '#a6adc8',
          accent: '#94e2d5',
          accent2: '#89b4fa',
          success: '#50fa7b',
          error: '#ff5555',
          warning: '#f9e2af',
          info: '#8be9fd'
        }
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'Helvetica', 'Arial', 'sans-serif'],
        mono: ['ui-monospace', 'SFMono-Regular', 'Monaco', 'Cascadia Code', 'Roboto Mono', 'Courier New', 'monospace']
      },
      animation: {
        'spin-slow': 'spin 1s linear infinite',
      }
    },
  },
  plugins: [],
}