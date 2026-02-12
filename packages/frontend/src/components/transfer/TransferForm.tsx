import { useState, useMemo, useEffect, useCallback } from 'react'
import { useAccount } from 'wagmi'
import { Address } from 'viem'
import { useWallet } from '../../hooks/useWallet'
import {
  useBridgeDeposit,
  computeTerraChainKey,
  computeEvmChainKey,
  encodeTerraAddress,
  encodeEvmAddress,
} from '../../hooks/useBridgeDeposit'
import { useTerraDeposit } from '../../hooks/useTerraDeposit'
import { useTransferStore } from '../../stores/transfer'
import { getChainById } from '../../lib/chains'
import { getChainsForTransfer } from '../../utils/bridgeChains'
import type { ChainInfo } from '../../lib/chains'
import type { TransferDirection } from '../../types/transfer'
import { DEFAULT_NETWORK, BRIDGE_CONFIG, DECIMALS, NETWORKS } from '../../utils/constants'
import { parseAmount, formatAmount } from '../../utils/format'
import { isValidAmount } from '../../utils/validation'
import { SourceChainSelector } from './SourceChainSelector'
import { DestChainSelector } from './DestChainSelector'
import { AmountInput } from './AmountInput'
import { RecipientInput } from './RecipientInput'
import { FeeBreakdown } from './FeeBreakdown'
import { SwapDirectionButton } from './SwapDirectionButton'

const TOKEN_CONFIGS: Record<string, { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined> = {
  local: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS
    ? {
        address: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS as Address,
        lockUnlockAddress: (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address,
        symbol: 'tLUNC',
        decimals: 6,
      }
    : undefined,
  testnet: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS
    ? {
        address: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS as Address,
        lockUnlockAddress: (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address,
        symbol: 'LUNC',
        decimals: 18,
      }
    : undefined,
  mainnet: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS
    ? {
        address: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS as Address,
        lockUnlockAddress: (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address,
        symbol: 'LUNC',
        decimals: 18,
      }
    : undefined,
}

function getChainIdNumeric(chainId: string, chains: ChainInfo[]): number {
  const c = chains.find((ch) => ch.id === chainId) ?? getChainById(chainId)
  if (!c) return 31337
  return typeof c.chainId === 'number' ? c.chainId : 31337
}

/** Derive the transfer direction from the selected source and dest chain types */
function deriveDirection(source: ChainInfo | undefined, dest: ChainInfo | undefined): TransferDirection {
  if (!source || !dest) return 'terra-to-evm'
  if (source.type === 'cosmos' && dest.type === 'evm') return 'terra-to-evm'
  if (source.type === 'evm' && dest.type === 'cosmos') return 'evm-to-terra'
  return 'evm-to-evm'
}

/** Get valid destination chains for a given source chain */
function getValidDestChains(allChains: ChainInfo[], sourceChainId: string): ChainInfo[] {
  const source = allChains.find((c) => c.id === sourceChainId)
  return allChains.filter((c) => {
    // Can't bridge to the same chain
    if (c.id === sourceChainId) return false
    // Cosmos → Cosmos not supported
    if (source?.type === 'cosmos' && c.type === 'cosmos') return false
    return true
  })
}

export function TransferForm() {
  const { isConnected: isEvmConnected, address: evmAddress } = useAccount()
  const { connected: isTerraConnected, address: terraAddress, luncBalance } = useWallet()
  const { recordTransfer } = useTransferStore()

  const allChains = useMemo(() => getChainsForTransfer(), [])

  // Default to Terra -> first EVM (or first chain if no EVM); anvil for local, bsc for mainnet/testnet
  const [sourceChain, setSourceChain] = useState(() => {
    const chains = getChainsForTransfer()
    const terra = chains.find((c) => c.type === 'cosmos')
    return terra?.id ?? chains[0]?.id ?? 'terra'
  })
  const [destChain, setDestChain] = useState(() => {
    const chains = getChainsForTransfer()
    const evm = chains.find((c) => c.type === 'evm')
    return evm?.id ?? chains[0]?.id ?? 'anvil'
  })
  const [amount, setAmount] = useState('')
  const [recipient, setRecipient] = useState('')
  const [txHash, setTxHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const sourceChainInfo = useMemo(
    () => allChains.find((c) => c.id === sourceChain) ?? getChainById(sourceChain),
    [allChains, sourceChain]
  )
  const destChainInfo = useMemo(
    () => allChains.find((c) => c.id === destChain) ?? getChainById(destChain),
    [allChains, destChain]
  )
  const direction = useMemo(() => deriveDirection(sourceChainInfo, destChainInfo), [sourceChainInfo, destChainInfo])

  // Compute valid destination chains based on source selection
  const destChains = useMemo(() => getValidDestChains(allChains, sourceChain), [allChains, sourceChain])

  // When source changes, ensure dest is still valid
  useEffect(() => {
    const validIds = destChains.map((c) => c.id)
    if (!validIds.includes(destChain) && validIds.length > 0) {
      setDestChain(validIds[0])
    }
  }, [destChains, destChain])

  const isSourceTerra = direction === 'terra-to-evm'
  const isDestEvm = direction === 'terra-to-evm' || direction === 'evm-to-evm'

  const tokenConfig = TOKEN_CONFIGS[DEFAULT_NETWORK]
  const {
    deposit: evmDeposit,
    status: evmStatus,
    depositTxHash,
    error: evmError,
    reset: resetEvm,
    tokenBalance,
  } = useBridgeDeposit(
    tokenConfig ? { tokenAddress: tokenConfig.address, lockUnlockAddress: tokenConfig.lockUnlockAddress } : undefined
  )
  const { lock: terraLock, status: terraStatus, txHash: terraTxHash, error: terraError, reset: resetTerra } = useTerraDeposit()

  const isWalletConnected = isSourceTerra ? isTerraConnected : isEvmConnected

  // Auto-fill recipient from connected wallet on the destination side
  const recipientAddr = useMemo(() => {
    if (recipient) return recipient
    if (isDestEvm) return evmAddress ?? ''
    return terraAddress ?? ''
  }, [recipient, isDestEvm, evmAddress, terraAddress])

  const receiveAmount = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return '0'
    const inputAmount = parseFloat(amount)
    const feeAmount = inputAmount * (BRIDGE_CONFIG.feePercent / 100)
    return (inputAmount - feeAmount).toFixed(6)
  }, [amount])

  const isSubmitting =
    evmStatus === 'checking-allowance' ||
    evmStatus === 'approving' ||
    evmStatus === 'waiting-approval' ||
    evmStatus === 'depositing' ||
    evmStatus === 'waiting-deposit' ||
    terraStatus === 'locking'

  useEffect(() => {
    if (evmStatus === 'success' && depositTxHash) {
      setTxHash(depositTxHash)
      recordTransfer({
        type: 'deposit',
        direction,
        sourceChain,
        destChain,
        amount: parseAmount(amount, tokenConfig?.decimals ?? DECIMALS.LUNC),
        status: 'confirmed',
        txHash: depositTxHash,
      })
      resetEvm()
    } else if (evmStatus === 'error' && evmError) {
      setError(evmError)
      resetEvm()
    }
  }, [evmStatus, depositTxHash, evmError, resetEvm, sourceChain, destChain, amount, tokenConfig, recordTransfer, direction])

  useEffect(() => {
    if (terraStatus === 'success' && terraTxHash) {
      setTxHash(terraTxHash)
      resetTerra()
    } else if (terraStatus === 'error' && terraError) {
      setError(terraError)
      resetTerra()
    }
  }, [terraStatus, terraTxHash, terraError, resetTerra])

  const handleSwap = useCallback(() => {
    const prevSource = sourceChain
    const prevDest = destChain
    setSourceChain(prevDest)
    setDestChain(prevSource)
    setError(null)
  }, [sourceChain, destChain])

  // Check if the swap would produce an invalid route (cosmos→cosmos)
  const isSwapDisabled = useMemo(() => {
    const destInfo = getChainById(destChain)
    const sourceInfo = getChainById(sourceChain)
    // Disable swap if swapping would result in cosmos→cosmos
    return destInfo?.type === 'cosmos' && sourceInfo?.type === 'cosmos'
  }, [sourceChain, destChain])

  const handleSourceChange = useCallback(
    (newSource: string) => {
      setSourceChain(newSource)
      setError(null)
    },
    []
  )

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setTxHash(null)

    if (!isWalletConnected || !amount || !isValidAmount(amount)) return

    if (direction === 'terra-to-evm') {
      // Terra → EVM: lock on Terra
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      const destChainId = getChainIdNumeric(destChain, allChains)
      const amountMicro = parseAmount(amount, DECIMALS.LUNC)
      await terraLock({ amountMicro, destChainId, recipientEvm: recipientAddr })
    } else if (direction === 'evm-to-terra') {
      // EVM → Terra: deposit on EVM router
      if (!recipientAddr || !recipientAddr.startsWith('terra1')) {
        setError('Please provide a Terra recipient address or connect your Terra wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      const destChainKey = computeTerraChainKey(NETWORKS[DEFAULT_NETWORK].terra.chainId)
      const destAccount = encodeTerraAddress(recipientAddr)
      await evmDeposit(amount, destChainKey, destAccount, tokenConfig.decimals)
    } else {
      // EVM → EVM: deposit on EVM router with EVM dest chain key
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      const destChainId = getChainIdNumeric(destChain, allChains)
      const destChainKey = computeEvmChainKey(destChainId)
      const destAccount = encodeEvmAddress(recipientAddr)
      await evmDeposit(amount, destChainKey, destAccount, tokenConfig.decimals)
    }
  }

  const balanceDisplay = isSourceTerra
    ? formatAmount(luncBalance, DECIMALS.LUNC)
    : tokenBalance !== undefined && tokenConfig
    ? formatAmount(tokenBalance.toString(), tokenConfig.decimals)
    : undefined

  const sourceLabel = isSourceTerra ? 'LUNC' : tokenConfig?.symbol
  const walletLabel = isSourceTerra ? 'Terra' : 'EVM'

  const buttonText = !isWalletConnected
    ? `Connect ${walletLabel} Wallet`
    : isSubmitting
    ? 'Processing...'
    : direction === 'terra-to-evm'
    ? 'Bridge from Terra'
    : direction === 'evm-to-evm'
    ? 'Bridge EVM to EVM'
    : 'Bridge from EVM'

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {txHash && (
        <div className="bg-green-900/30 border-2 border-green-700 p-3">
          <p className="text-green-300 text-xs font-semibold uppercase tracking-wide">Transaction submitted</p>
          <p className="text-green-500/70 text-xs mt-1 font-mono break-all">{txHash}</p>
        </div>
      )}
      {error && (
        <div className="bg-red-900/30 border-2 border-red-700 p-3">
          <p className="text-red-300 text-sm">{error}</p>
          <button type="button" onClick={() => setError(null)} className="text-red-200 text-xs mt-2 underline underline-offset-2">
            Dismiss
          </button>
        </div>
      )}

      <SourceChainSelector
        chains={allChains}
        value={sourceChain}
        onChange={handleSourceChange}
        balance={balanceDisplay}
        balanceLabel={sourceLabel}
      />
      <AmountInput
        value={amount}
        onChange={setAmount}
        onMax={
          isSourceTerra
            ? () => setAmount(formatAmount(luncBalance, DECIMALS.LUNC))
            : tokenBalance !== undefined && tokenConfig
            ? () => setAmount(formatAmount(tokenBalance.toString(), tokenConfig.decimals))
            : undefined
        }
        symbol="LUNC"
      />
      <SwapDirectionButton onClick={handleSwap} disabled={isSwapDisabled} />
      <DestChainSelector chains={destChains} value={destChain} onChange={setDestChain} />
      <RecipientInput value={recipient} onChange={setRecipient} direction={direction} />
      <FeeBreakdown receiveAmount={receiveAmount} />

      <button
        type="submit"
        disabled={!isWalletConnected || !amount || !isValidAmount(amount) || isSubmitting}
        className="btn-primary btn-cta w-full justify-center py-3 disabled:bg-gray-700 disabled:text-gray-400 disabled:shadow-none disabled:translate-x-0 disabled:translate-y-0 disabled:cursor-not-allowed"
      >
        {buttonText}
      </button>
    </form>
  )
}
