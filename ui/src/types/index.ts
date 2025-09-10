export interface StatusMessage {
  message: string
  type: 'info' | 'success' | 'error' | 'warning'
}

export interface WasmSectionInfo {
  name: string
  id: number
  size: number
}

export interface WasmInspectionInfo {
  valid_magic: boolean
  file_size: number
  section_count: number
  sections: WasmSectionInfo[]
  has_export_section: boolean
  export_names: string[]
  has_start_section: boolean
  start_function_index?: number
  has_memory_section: boolean
  memory_limits?: [number, number | null]
  has_table_section: boolean
  function_count: number
  plugin?: PluginInfo
}

export interface PluginCapabilities {
  compile_wasm: boolean
  compile_webapp: boolean
  live_reload: boolean
  optimization: boolean
  custom_targets: string[]
}

export interface PluginSource {
  type: 'crates.io' | 'local' | 'git'
  url?: string
  path?: string
  branch?: string
}

export interface PluginInfo {
  name: string
  version: string
  type: 'built-in' | 'external'
  description?: string
  author?: string
  source?: PluginSource
  capabilities?: PluginCapabilities
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
