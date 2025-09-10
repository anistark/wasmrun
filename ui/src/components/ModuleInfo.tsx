import { WasmModuleInfo } from '@/types'
import {
  ModuleDetailsCard,
  ExportsCard,
  ImportsCard,
  PluginCard,
  WasiSupportCard,
} from '@/components/modules'

interface ModuleInfoProps {
  moduleInfo: WasmModuleInfo | null
}


export function ModuleInfo({ moduleInfo }: ModuleInfoProps) {
  return (
    <div class="p-6 space-y-6">
      <div>
        <h2 class="text-2xl font-bold text-light-textPrimary dark:text-dark-textPrimary mb-2">
          WebAssembly Module Analysis
        </h2>
        <p class="text-light-textDim dark:text-dark-textDim">
          Comprehensive analysis of your WebAssembly module including binary inspection and plugin
          details
        </p>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        {/* Module Details with Analysis */}
        <div class="md:col-span-1 xl:col-span-1">
          <ModuleDetailsCard moduleInfo={moduleInfo} />
        </div>

        {/* Exports */}
        <div class="md:col-span-1 xl:col-span-1">
          <ExportsCard moduleInfo={moduleInfo} />
        </div>

        {/* Imports */}
        <div class="md:col-span-1 xl:col-span-1">
          <ImportsCard moduleInfo={moduleInfo} />
        </div>

        {/* Plugin Info */}
        <div class="md:col-span-1 xl:col-span-1">
          <PluginCard moduleInfo={moduleInfo} />
        </div>

        {/* WASI Support */}
        <div class="md:col-span-2 xl:col-span-2">
          <WasiSupportCard />
        </div>
      </div>
    </div>
  )
}
