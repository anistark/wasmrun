import { useState } from 'preact/hooks'
import { TabItem } from '@/types'
import clsx from 'clsx'

interface TabsProps {
  tabs: TabItem[]
  defaultTab?: string
}

export function Tabs({ tabs, defaultTab }: TabsProps) {
  const [activeTab, setActiveTab] = useState(defaultTab || tabs[0]?.id || '')

  const activeTabContent = tabs.find(tab => tab.id === activeTab)?.content

  return (
    <div class="max-w-4xl mx-auto my-6 rounded-lg overflow-hidden bg-dark-surface border border-dark-surface3">
      <div class="flex bg-dark-surface2 border-b border-dark-surface3">
        {tabs.map(tab => (
          <button
            key={tab.id}
            onClick={() => !tab.disabled && setActiveTab(tab.id)}
            class={clsx('px-6 py-3 text-sm font-medium transition-colors duration-200', {
              'bg-dark-surface border-b-2 border-dark-accent text-dark-textMuted':
                activeTab === tab.id,
              'text-dark-textMuted hover:bg-dark-surface3': activeTab !== tab.id && !tab.disabled,
              'text-dark-textDim cursor-not-allowed opacity-50': tab.disabled,
            })}
            disabled={tab.disabled}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div class="relative min-h-96">
        <div class="p-4">{activeTabContent}</div>
      </div>
    </div>
  )
}
