export interface StatusMessage {
  message: string
  type: 'info' | 'success' | 'error' | 'warning'
}

export interface WasmModuleInfo {
  name: string
  size: number
  imports: string[]
  exports: string[]
  isWasi: boolean
}

export interface LogEntry {
  timestamp: Date
  message: string
  type: 'info' | 'success' | 'error' | 'warning'
}

export interface FunctionParameter {
  name: string
  type: string
  value?: string
}

export interface ExportedFunction {
  name: string
  signature: string
  parameters: FunctionParameter[]
  description?: string
}

export interface TabItem {
  id: string
  label: string
  content: any
  disabled?: boolean
}

export type Theme = 'dark' | 'light'

export interface ThemeContextType {
  theme: Theme
  toggleTheme: () => void
}
