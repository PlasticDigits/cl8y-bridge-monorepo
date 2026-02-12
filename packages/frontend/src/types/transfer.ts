export type HashStatus = 'verified' | 'pending' | 'canceled' | 'fraudulent' | 'unknown'

export type TransferStatus = 'pending' | 'confirmed' | 'failed'

export type TransferDirection = 'evm-to-terra' | 'terra-to-evm'

export interface TransferRecord {
  id: string
  type: 'deposit' | 'withdrawal'
  direction: TransferDirection
  sourceChain: string
  destChain: string
  amount: string
  status: TransferStatus
  txHash: string
  timestamp: number
}

export interface TransferHash {
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
