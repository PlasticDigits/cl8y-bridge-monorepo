/**
 * Chain list loader - reads chainlist.json for chain display info (names, icons).
 * Used by chain dropdowns to show logos and info.
 */


export interface ChainlistEntry {
  id: string
  name: string
  chainId: number | string
  type: 'evm' | 'cosmos'
  icon: string
  rpcUrl?: string
  lcdUrl?: string
  explorerUrl?: string
  tier: 'local' | 'testnet' | 'mainnet'
}

export interface ChainlistData {
  name: string
  version: string
  chains: ChainlistEntry[]
}

/** Map BRIDGE_CHAINS ids to chainlist ids where they differ */
const ID_TO_CHAINLIST: Record<string, string> = {
  bsc: 'binancesmartchain',
  terra: 'terraclassic',
}

/** Load chainlist (imported at build time) */
import chainlistJson from '../../public/chains/chainlist.json'

const chainlistData: ChainlistData = chainlistJson as ChainlistData

/** Get chainlist data */
export function getChainlist(): ChainlistData {
  return chainlistData
}

/** Get chainlist entry by id or chainId. Resolves BRIDGE_CHAINS ids (bsc, terra) to chainlist ids. */
export function getChainlistEntry(
  chainlist: ChainlistData,
  bridgeId: string,
  chainId?: number | string
): ChainlistEntry | undefined {
  const chainlistId = ID_TO_CHAINLIST[bridgeId] ?? bridgeId
  let entry = chainlist.chains.find((c) => c.id === chainlistId || c.id === bridgeId)
  if (!entry && chainId !== undefined) {
    entry = chainlist.chains.find((c) => c.chainId === chainId)
  }
  return entry
}

/** Check if icon is an image path (not emoji) */
export function isIconImagePath(icon: string): boolean {
  return typeof icon === 'string' && (icon.startsWith('/') || icon.startsWith('http'))
}
