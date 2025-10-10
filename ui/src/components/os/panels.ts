import type { PanelType } from '../../types/osTypes'

export const panels: PanelType[] = [
  { id: 'project', name: 'Application', icon: '🌐' },
  { id: 'kernel', name: 'Kernel Status', icon: '⚙️' },
  { id: 'console', name: 'Console', icon: '📟' },
  { id: 'filesystem', name: 'File System', icon: '📁' },
  { id: 'processes', name: 'Processes', icon: '🔄' },
  { id: 'metrics', name: 'Metrics', icon: '📈' },
  { id: 'logs', name: 'Logs', icon: '📋' },
]
