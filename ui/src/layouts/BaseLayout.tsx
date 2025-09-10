import { ComponentChildren } from 'preact'
import { ThemeToggle } from '@/components/ThemeToggle'

interface BaseLayoutProps {
  title: string
  children: ComponentChildren
  showFooter?: boolean
}

export function BaseLayout({ title, children, showFooter = true }: BaseLayoutProps) {
  return (
    <div class="min-h-screen flex flex-col bg-light-bg dark:bg-dark-bg text-light-text dark:text-dark-text">
      <header class="flex items-center justify-between px-8 py-4 bg-light-surface dark:bg-dark-surface shadow-lg">
        <div class="flex items-center">
          <div class="flex items-center justify-center">
            <img src="/assets/logo.png" alt="Wasmrun Logo" width="40" height="40" class="w-10 h-10" />
          </div>
          <h1 class="ml-4 text-3xl font-semibold text-light-text dark:text-dark-text">{title}</h1>
        </div>
        <ThemeToggle />
      </header>

      <main class="flex-1 flex flex-col">{children}</main>

      {showFooter && (
        <footer class="bg-light-surface dark:bg-dark-surface py-4 text-center text-sm text-light-textMuted dark:text-dark-textMuted">
          Powered by Wasmrun
        </footer>
      )}
    </div>
  )
}
