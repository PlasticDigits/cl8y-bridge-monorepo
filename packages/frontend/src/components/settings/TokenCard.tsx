import type { TokenEntry } from '../../hooks/useTokenRegistry'
import { useTerraTokenDisplayInfo, useEvmTokensDisplayInfo } from '../../hooks/useTokenDisplayInfo'
import { useTokenChains } from '../../hooks/useTokenChains'
import { CopyButton, TokenLogo } from '../ui'
import { shortenAddress } from '../../utils/shortenAddress'

export interface TokenCardProps {
  token: TokenEntry
}

export function TokenCard({ token }: TokenCardProps) {
  const display = useTerraTokenDisplayInfo(token.token)
  const chains = useTokenChains(token.token, token.evm_token_address || undefined)
  const evmChains = chains.filter((c) => c.type === 'evm')
  const evmDisplayMap = useEvmTokensDisplayInfo(
    evmChains.map((c) => ({ address: c.address, rpcUrl: c.rpcUrl }))
  )

  // Native tokens use LockUnlock (locked on source, minted on dest)
  // CW20 tokens use MintBurn (minted/burned across chains)
  const bridgeMode = token.is_native ? 'LockUnlock' : 'MintBurn'

  const getChainDisplay = (c: (typeof chains)[0]) => {
    if (c.type === 'cosmos') return display.displayLabel
    const evmInfo = evmDisplayMap[c.address.toLowerCase()]
    return evmInfo?.displayLabel ?? shortenAddress(c.address)
  }

  return (
    <div className="border-2 border-white/20 bg-[#161616] p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <h4 className="flex items-center gap-2 font-medium text-white">
            <TokenLogo
              symbol={display.symbol}
              tokenId={token.token}
              addressForBlockie={display.addressForBlockie}
              size={24}
            />
            {display.displayLabel}
            {!token.enabled && (
              <span className="ml-2 text-xs text-amber-300">(disabled)</span>
            )}
          </h4>
          <p className="mt-1 text-sm text-gray-400">
            {bridgeMode} · Terra decimals: {token.terra_decimals} · EVM decimals: {token.evm_decimals}
          </p>
          <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs text-gray-400">
            {chains.map((c) => (
              <span key={c.chainId} className="flex items-center gap-1">
                <span>{c.chainName}:</span>
                <span className="font-mono text-gray-300">{getChainDisplay(c)}</span>
                {c.address && (
                  <CopyButton text={c.address} label={`Copy ${c.chainName} address`} />
                )}
              </span>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
