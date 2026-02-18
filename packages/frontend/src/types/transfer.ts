export type HashStatus = 'verified' | 'pending' | 'canceled' | 'fraudulent' | 'unknown'

export type TransferStatus = 'pending' | 'confirmed' | 'failed'

export type TransferDirection = 'evm-to-terra' | 'terra-to-evm' | 'evm-to-evm'

/**
 * Transfer lifecycle stages in the V2 bridge protocol:
 * 1. deposited    - Deposit/lock tx confirmed on source chain
 * 2. hash-submitted - withdrawSubmit called on destination chain
 * 3. approved     - Operator called withdrawApprove
 * 4. executed     - withdrawExecute completed, tokens released
 * 5. failed       - Any step failed
 */
export type TransferLifecycle =
  | 'deposited'
  | 'hash-submitted'
  | 'approved'
  | 'executed'
  | 'failed'

export interface TransferRecord {
  id: string
  type: 'deposit' | 'withdrawal'
  direction: TransferDirection
  sourceChain: string
  destChain: string
  amount: string
  status: TransferStatus
  txHash: string              // source chain deposit tx hash
  timestamp: number
  // V2 lifecycle tracking fields
  xchainHashId?: string       // keccak256 transfer hash (computed from deposit params)
  depositNonce?: number       // nonce from deposit event
  lifecycle?: TransferLifecycle
  withdrawSubmitTxHash?: string  // destination chain withdrawSubmit tx hash
  srcAccount?: string         // depositor address on source chain
  destAccount?: string        // recipient address on destination chain
  token?: string              // token identifier (denom or address)
  tokenSymbol?: string        // human-readable token symbol (e.g. "LUNC", "TKNA")
  srcDecimals?: number        // token decimals on source chain
  destToken?: string          // token address on destination chain (bytes32 or hex)
  destTokenId?: string        // raw destination token identifier (e.g. "uluna" for Terra, EVM address for EVM)
  destBridgeAddress?: string  // bridge contract address on destination chain
  sourceChainIdBytes4?: string // bytes4 hex of source chain (e.g. "0x00007a69")
}

export interface XchainHashId {
  hash: string
  srcChain: string
  destChain: string
  srcTxHash: string
  destTxHash: string | null
  srcAccount: string
  destAccount: string
  token: string
  amount: string
  nonce: string
  status: HashStatus
  canceledAt?: number
  cancelReason?: string
  fraudIndicators?: string[]
  createdAt: number
  updatedAt: number
}
