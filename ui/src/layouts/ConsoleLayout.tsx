import { ComponentChildren } from 'preact'
import { TabItem } from '@/types'
import { ThemeToggle } from '@/components/ThemeToggle'
import { useVersion } from '@/hooks/useVersion'
import clsx from 'clsx'

interface ConsoleLayoutProps {
  filename?: string
  children: ComponentChildren
  tabs?: TabItem[]
  activeTab?: string
  onTabChange?: (tabId: string) => void
}

export function ConsoleLayout({
  filename,
  children,
  tabs,
  activeTab,
  onTabChange,
}: ConsoleLayoutProps) {
  const { version, loading } = useVersion()

  return (
    <div class="h-screen flex flex-col bg-light-bg dark:bg-dark-bg text-light-text dark:text-dark-text overflow-hidden">
      <header class="bg-light-surface dark:bg-dark-surface shadow-lg flex-shrink-0">
        <div class="flex items-center justify-between px-8 py-4">
          <div class="flex items-center">
            <div class="flex items-center justify-center">
              <img
                src="/assets/logo-text.png"
                alt="Wasmrun"
                width="40"
                height="40"
                class="w-auto h-10"
              />
            </div>
          </div>
          <div class="flex-1 flex justify-center items-center">
            {filename && (
              <p class="text-sm text-light-textDim dark:text-dark-textDim mt-1">
                Running:{' '}
                <span class="font-mono text-green-500 dark:text-green-400">{filename}</span>
              </p>
            )}
          </div>
          <div class="flex items-center gap-4">
            <a
              href="https://github.com/anistark/wasmrun"
              target="_blank"
              title="GitHub"
              class="text-light-textDim dark:text-dark-textDim hover:text-light-accent2 dark:hover:text-purple-400 transition-colors"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
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
                x="0px"
                y="0px"
                width="20"
                height="20"
                viewBox="0,0,256,256"
              >
                <g
                  fill="#ffffff"
                  fill-rule="nonzero"
                  stroke="none"
                  stroke-width="1"
                  stroke-linecap="butt"
                  stroke-linejoin="miter"
                  stroke-miterlimit="10"
                  stroke-dasharray=""
                  stroke-dashoffset="0"
                  font-family="none"
                  font-weight="none"
                  font-size="none"
                  text-anchor="none"
                  style="mix-blend-mode: normal"
                >
                  <g transform="scale(5.12,5.12)">
                    <path d="M5.91992,6l14.66211,21.375l-14.35156,16.625h3.17969l12.57617,-14.57812l10,14.57813h12.01367l-15.31836,-22.33008l13.51758,-15.66992h-3.16992l-11.75391,13.61719l-9.3418,-13.61719zM9.7168,8h7.16406l23.32227,34h-7.16406z"></path>
                  </g>
                </g>
              </svg>
            </a>
            <ThemeToggle />
          </div>
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

      <main class="flex-1 flex flex-col min-h-0">{children}</main>

      <footer class="bg-light-surface dark:bg-dark-surface py-2 flex-shrink-0">
        <div class="px-8">
          <div class="flex justify-center items-center">
            <p class="text-xs font-semibold text-light-textDim dark:text-dark-textDim">
              Wasmrun{!loading && version && ` v${version}`}
            </p>
          </div>
        </div>
      </footer>
    </div>
  )
}
