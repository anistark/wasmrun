import { clsx } from 'clsx'
import type { PanelType } from '../../types/osTypes'

interface SidebarProps {
  panels: PanelType[]
  activePanel: string
  onPanelChange: (panelId: string) => void
}

export default function Sidebar({ panels, activePanel, onPanelChange }: SidebarProps) {
  const projectPanels = panels.slice(0, 2)
  const developmentPanels = panels.slice(2, 5)
  const monitoringPanels = panels.slice(5)

  return (
    <nav className="w-80 bg-black/20 backdrop-blur-lg border-r border-green-500/20 p-6">
      <div className="space-y-8">
        <PanelSection
          title="🎯 PROJECT"
          panels={projectPanels}
          activePanel={activePanel}
          onPanelChange={onPanelChange}
        />
        <PanelSection
          title="🔧 DEVELOPMENT"
          panels={developmentPanels}
          activePanel={activePanel}
          onPanelChange={onPanelChange}
        />
        <PanelSection
          title="📊 MONITORING"
          panels={monitoringPanels}
          activePanel={activePanel}
          onPanelChange={onPanelChange}
        />
      </div>
    </nav>
  )
}

interface PanelSectionProps {
  title: string
  panels: PanelType[]
  activePanel: string
  onPanelChange: (panelId: string) => void
}

function PanelSection({ title, panels, activePanel, onPanelChange }: PanelSectionProps) {
  return (
    <div>
      <h3 className="text-sm font-semibold text-green-400/90 mb-4 tracking-wide">{title}</h3>
      <div className="space-y-2">
        {panels.map(panel => (
          <button
            key={panel.id}
            onClick={() => onPanelChange(panel.id)}
            className={clsx(
              'w-full flex items-center gap-3 px-4 py-3 rounded-lg backdrop-blur-sm transition-all duration-200',
              {
                'bg-green-600/30 border border-green-400/50 text-white': activePanel === panel.id,
                'bg-white/5 border border-green-500/20 text-white/80 hover:bg-green-500/20 hover:text-white hover:translate-x-1':
                  activePanel !== panel.id,
              }
            )}
          >
            <span>{panel.icon}</span>
            <span className="font-medium">{panel.name}</span>
          </button>
        ))}
      </div>
    </div>
  )
}
