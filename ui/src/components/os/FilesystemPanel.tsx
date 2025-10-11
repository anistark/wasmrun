import { clsx } from 'clsx'
import type { FilesystemStats, DirEntry } from '../../types/osTypes'

interface FilesystemPanelProps {
  fsStats: FilesystemStats | null
  currentPath: string
  projectName: string
  dirEntries: DirEntry[]
  selectedFile: string | null
  fileContent: string
  isEditing: boolean
  onNavigateUp: () => void
  onRefresh: () => void
  onNavigate: (path: string) => void
  onFileSelect: (path: string) => void
  onEdit: () => void
  onSave: () => void
  onCancel: () => void
  onContentChange: (content: string) => void
  formatBytes: (bytes: number) => string
}

export default function FilesystemPanel({
  fsStats,
  currentPath,
  projectName,
  dirEntries,
  selectedFile,
  fileContent,
  isEditing,
  onNavigateUp,
  onRefresh,
  onNavigate,
  onFileSelect,
  onEdit,
  onSave,
  onCancel,
  onContentChange,
  formatBytes,
}: FilesystemPanelProps) {
  return (
    <div className="h-full flex flex-col">
      <div className="border-b border-green-500/20 bg-black/20 backdrop-blur-lg p-6">
        <h2 className="text-2xl font-bold mb-2 text-green-400">WASI File System</h2>
        <p className="text-white/80">Mounted directories and file operations</p>
      </div>
      <div className="flex-1 flex">
        {/* Left sidebar - file browser */}
        <div className="w-1/3 border-r border-green-500/20 bg-black/10 p-4 overflow-y-auto">
          <div className="mb-4">
            <div className="flex items-center gap-2 mb-2">
              <button
                onClick={onNavigateUp}
                disabled={currentPath === `/${projectName}`}
                className="px-3 py-1 bg-green-600/30 hover:bg-green-600/50 disabled:opacity-30 disabled:cursor-not-allowed border border-green-500/30 rounded text-sm"
              >
                ⬆️ Up
              </button>
              <button
                onClick={onRefresh}
                className="px-3 py-1 bg-green-600/30 hover:bg-green-600/50 border border-green-500/30 rounded text-sm"
              >
                🔄 Refresh
              </button>
            </div>
            <div className="text-sm text-green-400 font-mono mb-2">📂 {currentPath}</div>
          </div>

          <div className="space-y-1">
            {dirEntries.map(entry => (
              <button
                key={entry.name}
                onClick={() => {
                  const fullPath = `${currentPath}/${entry.name}`
                  if (entry.is_dir) {
                    onNavigate(fullPath)
                  } else {
                    onFileSelect(fullPath)
                  }
                }}
                className={clsx(
                  'w-full flex items-center justify-between px-3 py-2 rounded hover:bg-green-500/20 transition-colors text-left',
                  {
                    'bg-green-500/30': selectedFile === `${currentPath}/${entry.name}`,
                  }
                )}
              >
                <div className="flex items-center gap-2">
                  <span>{entry.is_dir ? '📁' : '📄'}</span>
                  <span className="text-sm font-mono">{entry.name}</span>
                </div>
                {entry.is_file && (
                  <span className="text-xs text-white/50">{formatBytes(entry.size)}</span>
                )}
              </button>
            ))}
          </div>

          {dirEntries.length === 0 && (
            <div className="text-center text-white/50 py-8">
              <div className="text-4xl mb-2">📂</div>
              <div>Empty directory</div>
            </div>
          )}
        </div>

        {/* Right panel - file viewer/editor and stats */}
        <div className="flex-1 flex flex-col">
          {/* Filesystem stats */}
          <div className="border-b border-green-500/20 bg-black/10 p-4">
            <div className="grid grid-cols-3 gap-4">
              <div className="bg-black/30 border border-green-500/30 rounded-lg p-3">
                <div className="text-xs text-green-400/80 mb-1">Mounted</div>
                <div className="text-xl font-bold">{fsStats?.total_mounts || 0}</div>
              </div>
              <div className="bg-black/30 border border-green-500/30 rounded-lg p-3">
                <div className="text-xs text-green-400/80 mb-1">Total Size</div>
                <div className="text-xl font-bold">{formatBytes(fsStats?.total_size || 0)}</div>
              </div>
              <div className="bg-black/30 border border-green-500/30 rounded-lg p-3">
                <div className="text-xs text-green-400/80 mb-1">Open FDs</div>
                <div className="text-xl font-bold">{fsStats?.open_fds || 0}</div>
              </div>
            </div>
          </div>

          {/* File content viewer/editor */}
          <div className="flex-1 p-4 overflow-hidden">
            {selectedFile ? (
              <div className="h-full flex flex-col">
                <div className="flex items-center justify-between mb-3">
                  <div className="text-sm text-green-400 font-mono">{selectedFile}</div>
                  <div className="flex gap-2">
                    {isEditing ? (
                      <>
                        <button
                          onClick={onSave}
                          className="px-3 py-1 bg-green-600 hover:bg-green-700 border border-green-400/50 rounded text-sm"
                        >
                          💾 Save
                        </button>
                        <button
                          onClick={onCancel}
                          className="px-3 py-1 bg-gray-600 hover:bg-gray-700 border border-gray-400/50 rounded text-sm"
                        >
                          ❌ Cancel
                        </button>
                      </>
                    ) : (
                      <button
                        onClick={onEdit}
                        className="px-3 py-1 bg-blue-600/80 hover:bg-blue-600 border border-blue-400/50 rounded text-sm"
                      >
                        ✏️ Edit
                      </button>
                    )}
                  </div>
                </div>
                <div className="flex-1 bg-black/50 border border-green-500/30 rounded-lg overflow-hidden">
                  {isEditing ? (
                    <textarea
                      value={fileContent}
                      onChange={e => onContentChange(e.currentTarget.value)}
                      className="w-full h-full bg-transparent text-white font-mono text-sm p-4 resize-none focus:outline-none"
                    />
                  ) : (
                    <pre className="w-full h-full text-white font-mono text-sm p-4 overflow-auto">
                      {fileContent}
                    </pre>
                  )}
                </div>
              </div>
            ) : (
              <div className="h-full flex items-center justify-center text-white/50">
                <div className="text-center">
                  <div className="text-6xl mb-4">📄</div>
                  <div>Select a file to view or edit</div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
