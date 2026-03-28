import { useEffect } from 'react'
import { useSolanaWalletStore } from '../stores/solanaWallet'
import { fetchNativeSolBalanceFormatted } from '../services/solana/connect'

const SOL_BALANCE_REFETCH_MS = 30_000

/**
 * Fetches native SOL into the wallet store for the header (and any other readers).
 * Mount once so we do not duplicate RPC polling when multiple hooks consume the store.
 */
export function SolanaWalletBalanceSync() {
  const connected = useSolanaWalletStore((s) => s.connected)
  const address = useSolanaWalletStore((s) => s.address)

  useEffect(() => {
    const { setBalance } = useSolanaWalletStore.getState()
    if (!connected || !address) {
      setBalance(null)
      return
    }

    let cancelled = false

    const refresh = async () => {
      try {
        const formatted = await fetchNativeSolBalanceFormatted(address)
        if (!cancelled) setBalance(formatted)
      } catch {
        if (!cancelled) setBalance(null)
      }
    }

    void refresh()
    const interval = window.setInterval(refresh, SOL_BALANCE_REFETCH_MS)
    return () => {
      cancelled = true
      window.clearInterval(interval)
    }
  }, [connected, address])

  return null
}
