import { ComponentChildren } from 'preact'

interface BaseLayoutProps {
  title: string
  children: ComponentChildren
  showFooter?: boolean
}

export function BaseLayout({ title, children, showFooter = true }: BaseLayoutProps) {
  return (
    <div class="min-h-screen flex flex-col bg-dark-bg text-dark-text">
      <header class="flex items-center px-8 py-4 bg-dark-surface shadow-lg">
        <div class="flex items-center justify-center">
          <img src="/assets/logo.png" alt="Wasmrun Logo" width="40" height="40" class="w-10 h-10" />
        </div>
        <h1 class="ml-4 text-3xl font-semibold text-dark-text">{title}</h1>
      </header>

      <main class="flex-1 flex flex-col">{children}</main>

      {showFooter && (
        <footer class="bg-dark-surface py-4 text-center text-sm text-dark-textMuted">
          Powered by Wasmrun
        </footer>
      )}
    </div>
  )
}
