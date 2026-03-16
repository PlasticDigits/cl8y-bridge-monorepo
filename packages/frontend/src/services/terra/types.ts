export type TerraWalletType = 'station' | 'keplr' | 'luncdash' | 'galaxy' | 'leap' | 'cosmostation'

interface KeplrLikeExtension {
  enable: (chainIds: string | string[]) => Promise<void>
  getKey: (chainId: string) => Promise<{
    name: string
    bech32Address: string
    pubKey: Uint8Array
    isNanoLedger: boolean
  }>
  getOfflineSigner: (chainId: string) => unknown
  experimentalSuggestChain?: (chainInfo: unknown) => Promise<void>
}

declare global {
  interface Window {
    station?: {
      connect: () => Promise<void>
      disconnect: () => Promise<void>
      keplr?: KeplrLikeExtension
    }
    keplr?: KeplrLikeExtension
    leap?: KeplrLikeExtension
    cosmostation?: {
      providers: {
        keplr: KeplrLikeExtension | unknown
      }
    }
  }
}
