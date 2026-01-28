/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_EVM_RPC_URL: string
  readonly VITE_TERRA_LCD_URL: string
  readonly VITE_EVM_BRIDGE_ADDRESS: string
  readonly VITE_TERRA_BRIDGE_ADDRESS: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
