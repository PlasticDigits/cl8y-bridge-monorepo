/// <reference types="vite/client" />

declare module 'react-blockies'

interface ImportMetaEnv {
  readonly VITE_EVM_RPC_URL: string
  readonly VITE_TERRA_LCD_URL: string
  readonly VITE_EVM_BRIDGE_ADDRESS: string
  readonly VITE_TERRA_BRIDGE_ADDRESS: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

declare const __GIT_SHA__: string
declare const __APP_VERSION__: string
