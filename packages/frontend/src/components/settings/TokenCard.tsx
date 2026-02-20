import type { TokenEntry } from '../../hooks/useTokenRegistry'
import type { TokenChainInfo } from '../../hooks/useTokenChains'
import { useTerraTokenDisplayInfo } from '../../hooks/useTokenDisplayInfo'
import { useTokenChains } from '../../hooks/useTokenChains'
import { useTokenList } from '../../hooks/useTokenList'
import { CopyButton, TokenLogo } from '../ui'
import { shortenAddress } from '../../utils/shortenAddress'
import { getTokenExplorerUrl } from '../../utils/format'
import { getChainDisplayInfo } from '../../utils/bridgeChains'
import { isIconImagePath } from '../../utils/chainlist'
import { getTerraAddressFromList } from '../../services/tokenlist'

export interface TokenCardProps {
  token: TokenEntry
}

function ChainIcon({ chainId }: { chainId: string }) {
  try {
    const display = getChainDisplayInfo(chainId)
    return isIconImagePath(display.icon) ? (
      <img src={display.icon} alt="" className="h-5 w-5 shrink-0 rounded-full object-contain" />
    ) : (
      <span className="text-base">{display.icon}</span>
    )
  } catch {
    return <span className="text-base">○</span>
  }
}

/** Resolve Terra address for display: prefer terra1xxx, resolve symbol via tokenlist when needed */
function terraAddressForDisplay(
  c: TokenChainInfo,
  tokenId: string,
  symbol: string,
  tokenlist: { tokens: Array<{ symbol?: string; address?: string; denom?: string; type?: string }> } | null
): string {
  if (c.type !== 'cosmos' || !c.address) return c.address || ''
  const addr = c.address.trim()
  // Already terra1 address
  if (addr.startsWith('terra1')) return addr
  // cw20:terra1xxx format – extract terra1 part
  const terraMatch = addr.match(/terra1[a-z0-9]+/i)
  if (terraMatch) return terraMatch[0]!
  // Symbol-like (tdec etc): resolve via tokenlist to get terra1 address
  const resolved = getTerraAddressFromList(tokenlist, tokenId, symbol)
  if (resolved) return resolved
  return addr
}

export function TokenCard({ token }: TokenCardProps) {
  const display = useTerraTokenDisplayInfo(token.token)
  const chains = useTokenChains(token.token, token.evm_token_address || undefined)
  const { data: tokenlist } = useTokenList()

  // Native tokens use LockUnlock (locked on source, minted on dest)
  // CW20 tokens use MintBurn (minted/burned across chains)
  const bridgeMode = token.is_native ? 'LockUnlock' : 'MintBurn'

  const getExplorerLink = (c: TokenChainInfo, displayAddr: string) => {
    if (!displayAddr || !c.explorerUrl) return null
    if (c.type === 'cosmos' && !displayAddr.startsWith('terra1')) return null
    const url = getTokenExplorerUrl(c.explorerUrl, displayAddr, c.type)
    if (!url) return null
    return (
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        className="shrink-0 text-cyan-300 transition-colors hover:text-cyan-200"
        title={`View on ${c.chainName} explorer`}
      >
        ↗
      </a>
    )
  }

  const shortAddr = (addr: string) => (addr ? shortenAddress(addr) : '—')
  const decimalsForChain = (c: TokenChainInfo) =>
    c.type === 'cosmos' ? token.terra_decimals : token.evm_decimals

  return (
    <div className="border-2 border-white/20 bg-[#161616]">
      <div className="border-b border-white/20 bg-[#161616] px-4 py-3">
        <h4 className="flex items-center gap-2 font-medium text-white">
          <TokenLogo
            symbol={display.symbol}
            tokenId={token.token}
            addressForBlockie={display.addressForBlockie}
            size={24}
          />
          <span className="flex items-center gap-1.5">
            {display.symbol}
            {display.name && (
              <>
                <span className="text-gray-500" aria-hidden>—</span>
                <span className="text-gray-400">{display.name}</span>
              </>
            )}
          </span>
          {bridgeMode && (
            <span className="ml-2 text-xs text-gray-500">({bridgeMode})</span>
          )}
          {!token.enabled && (
            <span className="ml-2 text-xs text-amber-300">(disabled)</span>
          )}
        </h4>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-white/20 bg-[#161616]">
              <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">
                Chain
              </th>
              <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">
                Address
              </th>
              <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">
                Decimals
              </th>
            </tr>
          </thead>
          <tbody>
            {chains.map((c) => {
              const displayAddr =
                c.type === 'cosmos'
                  ? terraAddressForDisplay(c, token.token, display.symbol, tokenlist ?? null)
                  : c.address
              const copyVal = c.type === 'cosmos' ? displayAddr : c.address
              return (
                <tr key={c.chainId} className="border-b border-white/10">
                  <td className="px-4 py-2">
                    <span className="flex items-center gap-2 text-gray-300">
                      <ChainIcon chainId={c.chainId} />
                      {c.chainName}
                    </span>
                  </td>
                  <td className="px-4 py-2">
                    <span className="flex items-center gap-1.5">
                      <span className="font-mono text-gray-300">
                        {shortAddr(displayAddr)}
                      </span>
                      {getExplorerLink(c, displayAddr)}
                      {copyVal && (
                        <CopyButton text={copyVal} label={`Copy ${c.chainName} address`} />
                      )}
                    </span>
                  </td>
                  <td className="px-4 py-2 font-mono text-gray-400">
                    {decimalsForChain(c)}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
