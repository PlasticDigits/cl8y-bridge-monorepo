export type TerraWalletType = 'station' | 'keplr' | 'luncdash' | 'galaxy' | 'leap' | 'cosmostation'

declare global {
  interface Window {
    station?: {
      connect: () => Promise<void>
      disconnect: () => Promise<void>
    }
    keplr?: {
      enable: (chainId: string) => Promise<void>
      getOfflineSigner: (chainId: string) => unknown
    }
    leap?: {
      enable: (chainId: string) => Promise<void>
      getOfflineSigner: (chainId: string) => unknown
    }
    cosmostation?: {
      providers: {
        keplr: unknown
      }
    }
  }
}
