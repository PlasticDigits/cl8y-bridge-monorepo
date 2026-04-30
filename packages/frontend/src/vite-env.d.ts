/// <reference types="vite/client" />

declare module 'react-blockies'

interface ImportMetaEnv {
  /** When "true", all routes show a static under-construction page (no bridge UI). */
  readonly VITE_UNDER_CONSTRUCTION?: string
  readonly VITE_EVM_RPC_URL: string
  readonly VITE_TERRA_LCD_URL: string
  readonly VITE_TERRA_RPC_URL: string
  readonly VITE_EVM_BRIDGE_ADDRESS: string
  readonly VITE_TERRA_BRIDGE_ADDRESS: string
  /** MegaETH mainnet (GL-124); mirror operators’ `MEGAETH_*` for browser builds (`VITE_` prefix required). */
  readonly VITE_MEGAETH_RPC_URL?: string
  readonly VITE_MEGAETH_BRIDGE_ADDRESS?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

declare const __GIT_SHA__: string
declare const __APP_VERSION__: string
