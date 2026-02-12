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
    <div className="bg-gray-900/50 rounded-lg border border-gray-700/50 p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <h4 className="font-medium text-white">
            {token.token}
            {!token.enabled && (
              <span className="ml-2 text-xs text-yellow-500">(disabled)</span>
            )}
          </h4>
          <p className="text-gray-500 text-sm mt-1">
            {bridgeMode} · Terra decimals: {token.terra_decimals} · EVM decimals: {token.evm_decimals}
          </p>
          {token.evm_token_address && (
            <div className="flex items-center gap-2 mt-2">
              <span className="text-gray-400 text-xs font-mono truncate max-w-[200px]">
                EVM: {token.evm_token_address}
              </span>
              <CopyButton text={token.evm_token_address} label="Copy EVM address" />
            </div>
          )}
          <p className="text-gray-500 text-xs mt-2">
            Chains: {chains.join(', ')}
          </p>
        </div>
      </div>
    </div>
  )
}
