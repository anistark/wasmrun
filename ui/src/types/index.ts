export interface StatusMessage {
  message: string
  type: 'info' | 'success' | 'error' | 'warning'
}

export interface WasmSectionInfo {
  name: string
  id: number
  size: number
  offset: string
}

export interface WasmInspectionInfo {
  magicBytes: string
  version: number
  sections: WasmSectionInfo[]
  totalSections: number
  isValid: boolean
  warnings: string[]
}

export interface PluginInfo {
  name: string
  version: string
  type: 'built-in' | 'external'
  description?: string
}

export interface WasmModuleInfo {
  name: string
  size: number
  imports: string[]
  exports: string[]
  isWasi: boolean
  plugin?: PluginInfo
  inspection?: WasmInspectionInfo
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
