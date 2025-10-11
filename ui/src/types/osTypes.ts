export interface KernelStats {
  status: string
  active_processes: number
  total_memory_usage: number
  active_runtimes: string[]
  active_dev_servers: number
  project_pid: number | null
  // System information
  os: string
  arch: string
  kernel_version: string
  // WASI capabilities
  wasi_capabilities: string[]
  filesystem_mounts: number
  supported_languages: string[]
}

export interface FilesystemStats {
  total_mounts: number
  total_size: number
  open_fds: number
  mounts: Array<{
    guest_path: string
    host_path: string
    size: number
  }>
}

export interface DirEntry {
  name: string
  is_dir: boolean
  is_file: boolean
  size: number
}

export interface PanelType {
  id: string
  name: string
  icon: string
}

export type StatusType = 'loading' | 'running' | 'error'
