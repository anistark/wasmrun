import { ComponentChildren } from 'preact'
import { TabItem } from '@/types'
import { ThemeToggle } from '@/components/ThemeToggle'
import clsx from 'clsx'

interface ConsoleLayoutProps {
  title: string
  children: ComponentChildren
  tabs?: TabItem[]
  activeTab?: string
  onTabChange?: (tabId: string) => void
}

export function ConsoleLayout({
  title,
  children,
  tabs,
  activeTab,
  onTabChange,
}: ConsoleLayoutProps) {
  return (
    <div class="min-h-screen flex flex-col bg-light-bg dark:bg-dark-bg text-light-text dark:text-dark-text">
      <header class="bg-light-surface dark:bg-dark-surface shadow-lg">
        <div class="flex items-center justify-between px-8 py-4">
          <div class="flex items-center">
            <div class="flex items-center justify-center">
              <img
                src="/assets/logo.png"
                alt="Wasmrun Logo"
                width="40"
                height="40"
                class="w-10 h-10"
              />
            </div>
            <h1 class="ml-4 text-3xl font-semibold text-light-text dark:text-dark-text">{title}</h1>
          </div>
          <ThemeToggle />
        </div>
        {tabs && tabs.length > 0 && (
          <div class="flex bg-light-surface2 dark:bg-dark-surface2 border-t border-light-surface3 dark:border-dark-surface3">
            {tabs.map(tab => (
              <button
                key={tab.id}
                onClick={() => !tab.disabled && onTabChange?.(tab.id)}
                class={clsx('px-6 py-3 text-sm font-medium transition-colors duration-200', {
                  'bg-light-surface dark:bg-dark-surface border-b-2 border-light-accent2 dark:border-dark-accent text-light-textMuted dark:text-dark-textMuted':
                    activeTab === tab.id,
                  'text-light-textMuted dark:text-dark-textMuted hover:bg-light-surface3 dark:hover:bg-dark-surface3':
                    activeTab !== tab.id && !tab.disabled,
                  'text-light-textDim dark:text-dark-textDim cursor-not-allowed opacity-50':
                    tab.disabled,
                })}
                disabled={tab.disabled}
              >
                {tab.label}
              </button>
            ))}
          </div>
        )}
      </header>

      <main class="flex-1 flex flex-col">{children}</main>

      <footer class="bg-light-surface dark:bg-dark-surface py-8 mt-8">
        <div class="max-w-4xl mx-auto px-8">
          <div class="flex flex-wrap justify-between items-center">
            <div class="flex items-center mb-4 lg:mb-0">
              <img
                src="/assets/logo.png"
                alt="Wasmrun Logo"
                width="32"
                height="32"
                class="w-8 h-8 mr-2"
              />
              <span class="text-lg font-semibold text-light-textMuted dark:text-dark-textMuted">
                Wasmrun
              </span>
            </div>

            <p class="text-sm text-light-textDim dark:text-dark-textDim mb-4 lg:mb-0">
              Powered by Wasmrun
            </p>

            <div class="flex gap-4">
              <a
                href="https://github.com/anistark/wasmrun"
                target="_blank"
                title="GitHub"
                class="text-light-textDim dark:text-dark-textDim hover:text-light-accent2 dark:hover:text-purple-400 transition-colors"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="24"
                  height="24"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"></path>
                </svg>
              </a>
              <a
                href="https://x.com/kranirudha"
                target="_blank"
                title="Twitter"
                class="text-light-textDim dark:text-dark-textDim hover:text-light-accent2 dark:hover:text-purple-400 transition-colors"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="24"
                  height="24"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M23 3a10.9 10.9 0 0 1-3.14 1.53 4.48 4.48 0 0 0-7.86 3v1A10.66 10.66 0 0 1 3 4s-4 9 5 13a11.64 11.64 0 0 1-7 2c9 5 20 0 20-11.5a4.5 4.5 0 0 0-.08-.83A7.72 7.72 0 0 0 23 3z"></path>
                </svg>
              </a>
            </div>
          </div>
        </div>
      </footer>
    </div>
  )
}
