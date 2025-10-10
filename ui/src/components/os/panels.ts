import type { PanelType } from '../../types/osTypes'

export const panels: PanelType[] = [
  { id: 'project', name: 'Application', icon: '🌐' },
  { id: 'kernel', name: 'Kernel Status', icon: '⚙️' },
  { id: 'console', name: 'Console (Coming Soon)', icon: '📟' },
  { id: 'filesystem', name: 'File System', icon: '📁' },
  { id: 'processes', name: 'Processes (Coming Soon)', icon: '🔄' },
  { id: 'metrics', name: 'Metrics (Coming Soon)', icon: '📈' },
  { id: 'logs', name: 'Logs (Coming Soon)', icon: '📋' },
]
