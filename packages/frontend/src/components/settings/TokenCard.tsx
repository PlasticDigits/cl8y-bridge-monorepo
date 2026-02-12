import type { TokenEntry } from '../../hooks/useTokenRegistry'
import { CopyButton } from '../ui'

export interface TokenCardProps {
  token: TokenEntry
}

export function TokenCard({ token }: TokenCardProps) {
  // Native tokens use LockUnlock (locked on source, minted on dest)
  // CW20 tokens use MintBurn (minted/burned across chains)
  const bridgeMode = token.is_native ? 'LockUnlock' : 'MintBurn'

  // Determine registered chains based on available addresses
  const chains: string[] = ['Terra']
  if (token.evm_token_address) {
    chains.push('EVM')
  }

  return (
    <div className="border-2 border-white/20 bg-[#161616] p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <h4 className="font-medium text-white">
            {token.token}
            {!token.enabled && (
              <span className="ml-2 text-xs text-amber-300">(disabled)</span>
            )}
          </h4>
          <p className="mt-1 text-sm text-gray-400">
            {bridgeMode} · Terra decimals: {token.terra_decimals} · EVM decimals: {token.evm_decimals}
          </p>
          {token.evm_token_address && (
            <div className="flex items-center gap-2 mt-2">
              <span className="max-w-[200px] truncate font-mono text-xs text-gray-300">
                EVM: {token.evm_token_address}
              </span>
              <CopyButton text={token.evm_token_address} label="Copy EVM address" />
            </div>
          )}
          <p className="mt-2 text-xs text-gray-400">
            Chains: {chains.join(', ')}
          </p>
        </div>
      </div>
    </div>
  )
}
