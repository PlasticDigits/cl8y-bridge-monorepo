import { getAccount } from 'wagmi/actions'
import { config } from '../lib/wagmi'

/**
 * Poll wagmi account until the active EVM chain matches `destChainId`, or timeout.
 * Used after `switchChainAsync` resolves but connector state may lag (GL-131).
 */
export async function waitForWalletChainId(
  destChainId: number,
  opts?: { timeoutMs?: number; pollMs?: number },
): Promise<boolean> {
  const timeoutMs = opts?.timeoutMs ?? 30_000
  const pollMs = opts?.pollMs ?? 150
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const { chainId } = getAccount(config)
    if (chainId === destChainId) return true
    await new Promise((r) => setTimeout(r, pollMs))
  }
  return false
}
