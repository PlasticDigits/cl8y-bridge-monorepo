/**
 * Transfer Status Page
 *
 * Route: /transfer/:xchainHashId
 *
 * Shows a stepper UI tracking the full V2 bridge transfer lifecycle:
 *   1. Deposit (confirmed on source chain)
 *   2. Submit Hash (withdrawSubmit on destination chain)
 *   3. Approval (operator withdrawApprove)
 *   4. Execution (operator withdrawExecute, tokens released)
 *
 * Auto-submits withdrawSubmit when both wallets are connected.
 * Falls back to manual submit with instructions.
 */

import { useParams, Link } from 'react-router-dom'
import { useMemo, useEffect, useState, useCallback, useRef } from 'react'
import { useAccount, useSwitchChain } from 'wagmi'
import { useTransferStore } from '../stores/transfer'
import { useAutoWithdrawSubmit } from '../hooks/useAutoWithdrawSubmit'
import { useMultiChainLookup } from '../hooks/useMultiChainLookup'
import { useBrokenTransferFix } from '../hooks/useBrokenTransferFix'
import { useWithdrawSubmit } from '../hooks/useWithdrawSubmit'
import { useWallet } from '../hooks/useWallet'
import { hexToUint8Array } from '../services/terra/withdrawSubmit'
import { useApprovalCountdown } from '../hooks/useApprovalCountdown'
import { useBridgeConfig } from '../hooks/useBridgeConfig'
import { useStepProgress } from '../hooks/useStepProgress'
import { formatCountdownMmSs, formatCancelWindowRange } from '../utils/format'
import { parseTerraLockReceipt } from '../services/terra/depositReceipt'
import { computeXchainHashId, chainIdToBytes32, evmAddressToBytes32, terraAddressToBytes32 } from '../services/hashVerification'
import { isValidXchainHashId, normalizeXchainHashId } from '../utils/validation'
import {
  BRIDGE_CHAINS,
  getBridgeChainEntryByBytes4,
  getChainKeyByConfig,
  type NetworkTier,
} from '../utils/bridgeChains'
import type { BridgeChainConfig } from '../types/chain'
import { DEFAULT_NETWORK } from '../utils/constants'
import { sounds } from '../lib/sounds'
import type { TransferRecord, TransferLifecycle } from '../types/transfer'

const LOG = '[TransferStatus]'

type NonceResolutionStatus = 'idle' | 'resolving' | 'resolved' | 'failed'

// Lifecycle step definitions
const STEPS: { key: TransferLifecycle; label: string; description: string }[] = [
  { key: 'deposited', label: 'Deposit', description: 'Tokens locked on source chain' },
  { key: 'hash-submitted', label: 'Submit Hash', description: 'Withdrawal submitted to destination' },
  { key: 'approved', label: 'Approval', description: 'Operator verified deposit' },
  { key: 'executed', label: 'Complete', description: 'Tokens delivered to recipient' },
]

const LIFECYCLE_ORDER: TransferLifecycle[] = ['deposited', 'hash-submitted', 'approved', 'executed']

function getStepIndex(lifecycle?: TransferLifecycle): number {
  if (!lifecycle || lifecycle === 'failed') return 0
  const idx = LIFECYCLE_ORDER.indexOf(lifecycle)
  if (idx < 0) return 0
  // When executed (complete), use STEPS.length so the final step shows as DONE
  if (lifecycle === 'executed') return STEPS.length
  return idx
}

function StepIndicator({
  step,
  currentIdx,
  idx,
  isFailed,
}: {
  step: typeof STEPS[number]
  currentIdx: number
  idx: number
  isFailed: boolean
}) {
  const isDone = idx < currentIdx
  const isActive = idx === currentIdx
  const isError = isFailed && isActive
  const progress = useStepProgress(step.key, isDone, isActive && !isError)

  const stateLabel = isDone ? 'DONE' : isError ? 'FAILED' : isActive ? 'ACTIVE' : 'UP NEXT'
  const squareTone = isDone
    ? 'border-emerald-600 bg-emerald-950/50'
    : isError
    ? 'border-red-600 bg-red-950/70'
    : isActive
    ? 'border-yellow-500 bg-yellow-950/70'
    : 'border-white/35 bg-[#111111]'
  const statePillTone = isDone
    ? 'border-emerald-600/70 bg-emerald-950/40 text-emerald-300'
    : isError
    ? 'border-red-600/70 bg-red-900/40 text-red-300'
    : isActive
    ? 'border-yellow-600/80 bg-yellow-900/40 text-yellow-300'
    : 'border-white/25 bg-[#1c1c1c] text-gray-400'
  const connectorTone = isDone ? 'bg-emerald-600' : isError ? 'bg-red-500' : 'bg-gray-600'
  const arrowTone = isDone ? 'border-t-emerald-600' : isError ? 'border-t-red-500' : 'border-t-gray-500'
  const iconSrc = isDone
    ? '/assets/status-success.png'
    : isError
    ? '/assets/status-failed.png'
    : isActive
    ? '/assets/status-pending.png'
    : '/assets/status-canceled.png'
  const cardTone = isActive || isError ? 'border-white/70' : 'border-white/35'
  const textTone = isDone ? 'text-emerald-400' : isError ? 'text-red-300' : isActive ? 'text-white' : 'text-gray-400'
  const descriptionTone = isDone || isActive || isError ? 'text-gray-300' : 'text-gray-500'

  const barColor = isDone
    ? 'bg-emerald-600'
    : isError
    ? 'bg-red-500'
    : isActive
    ? 'bg-yellow-500'
    : 'bg-gray-700'
  const showBar = isDone || isActive || isError

  return (
    <div className={`relative flex items-start gap-3 border-2 bg-[#161616] p-3 shadow-[3px_3px_0_#000] ${cardTone}`}>
      {/* Status square + connector */}
      <div className="flex self-stretch flex-col items-center">
        <div
          className={`flex h-9 w-9 shrink-0 items-center justify-center border-2 shadow-[2px_2px_0_#000] ${squareTone}`}
        >
          <img src={iconSrc} alt="" className="h-5 w-5 shrink-0 object-contain" aria-hidden />
        </div>
        {idx < STEPS.length - 1 && (
          <div className="mt-1 flex min-h-6 flex-1 flex-col items-center">
            <div className={`w-1 flex-1 border-x border-black/50 ${connectorTone}`} />
            <div
              className={`mt-0.5 h-0 w-0 border-l-[5px] border-r-[5px] border-t-[7px] border-l-transparent border-r-transparent ${arrowTone}`}
            />
          </div>
        )}
      </div>
      {/* Text + progress */}
      <div className="flex-1 pb-1">
        <p
          className={`mb-1 inline-flex border-2 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide shadow-[1px_1px_0_#000] ${statePillTone}`}
        >
          {stateLabel}
        </p>
        <p className={`text-sm font-semibold uppercase tracking-[0.08em] ${textTone}`}>
          {step.label}
        </p>
        <p className={`text-xs ${descriptionTone}`}>
          {step.description}
        </p>
        {showBar && (
          <div className="mt-2 h-1.5 w-full overflow-hidden border border-white/15 bg-black/50">
            <div
              className={`relative h-full overflow-hidden transition-[width] duration-300 ease-out ${barColor}`}
              style={{ width: `${Math.round(progress)}%` }}
            >
              {isActive && !isError && (
                <div className="animate-progress-shimmer absolute inset-y-0 w-2/5 bg-gradient-to-r from-transparent via-white/60 to-transparent" />
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function TransferDetails({ transfer }: { transfer: TransferRecord }) {
  const [copied, setCopied] = useState(false)

  const copyHash = useCallback(() => {
    if (transfer.xchainHashId) {
      navigator.clipboard.writeText(transfer.xchainHashId)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }, [transfer.xchainHashId])

  return (
    <div className="space-y-3 text-sm">
      {/* XChain Hash ID */}
      {transfer.xchainHashId && (
        <div>
          <p className="text-xs uppercase tracking-wide text-gray-400 mb-1">XChain Hash ID</p>
          <div className="flex items-center gap-2">
            <code className="flex-1 truncate font-mono text-xs text-gray-200 bg-black/40 border border-gray-700 px-2 py-1">
              {transfer.xchainHashId}
            </code>
            <button
              type="button"
              onClick={copyHash}
              className="btn-muted px-2 py-1 text-xs"
            >
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
        </div>
      )}

      {/* Direction */}
      <div className="flex gap-4">
        <div>
          <p className="text-xs uppercase tracking-wide text-gray-400">From</p>
          <p className="text-white">{transfer.sourceChain}</p>
        </div>
        <div className="flex items-end text-gray-500 pb-0.5">&rarr;</div>
        <div>
          <p className="text-xs uppercase tracking-wide text-gray-400">To</p>
          <p className="text-white">{transfer.destChain}</p>
        </div>
      </div>

      {/* Amount */}
      <div>
        <p className="text-xs uppercase tracking-wide text-gray-400">Amount</p>
        <p className="text-white">{transfer.amount}</p>
      </div>

      {/* Source tx */}
      <div>
        <p className="text-xs uppercase tracking-wide text-gray-400">Source TX</p>
        <code className="block truncate font-mono text-xs text-gray-300">{transfer.txHash}</code>
      </div>

      {/* Withdraw submit tx */}
      {transfer.withdrawSubmitTxHash && (
        <div>
          <p className="text-xs uppercase tracking-wide text-gray-400">Destination TX</p>
          <code className="block truncate font-mono text-xs text-gray-300">
            {transfer.withdrawSubmitTxHash}
          </code>
        </div>
      )}

      {/* Recipient */}
      {transfer.destAccount && (
        <div>
          <p className="text-xs uppercase tracking-wide text-gray-400">Recipient</p>
          <code className="block truncate font-mono text-xs text-gray-300">
            {transfer.destAccount}
          </code>
        </div>
      )}
    </div>
  )
}

/** Extract bytes4 from bytes32 (left-aligned, e.g. 0x00000038... → 0x00000038) */
function bytes32ToBytes4(bytes32: `0x${string}`): string {
  const clean = bytes32.slice(2).padStart(64, '0')
  return ('0x' + clean.slice(0, 8)) as string
}

function buildTransferFromLookup(
  hash: string,
  source: {
    amount: bigint
    srcAccount: `0x${string}`
    destAccount: `0x${string}`
    token: `0x${string}`
    srcChain?: `0x${string}`
    destChain?: `0x${string}`
    nonce?: bigint
  } | null,
  sourceChain: BridgeChainConfig | null,
  dest: { amount: bigint; executed: boolean; approved: boolean } | null,
  destChain: BridgeChainConfig | null
): TransferRecord {
  const lifecycle: TransferRecord['lifecycle'] = dest?.executed
    ? 'executed'
    : dest?.approved
    ? 'approved'
    : dest
    ? 'hash-submitted'
    : 'deposited'
  const amount = (source?.amount ?? dest?.amount ?? 0n).toString()
  const srcIsCosmos = sourceChain?.type === 'cosmos'
  const destIsCosmos = destChain?.type === 'cosmos'
  const direction: TransferRecord['direction'] = srcIsCosmos ? 'terra-to-evm' : destIsCosmos ? 'evm-to-terra' : 'evm-to-evm'

  // Resolve chain KEYs (e.g. 'bsc', 'terra') instead of display names
  const resolvedSourceChainKey = sourceChain ? (getChainKeyByConfig(sourceChain) ?? sourceChain.name ?? 'unknown') : 'unknown'
  let resolvedSourceBytes4 = sourceChain?.bytes4ChainId
  if (!resolvedSourceBytes4 && source?.srcChain) {
    resolvedSourceBytes4 = bytes32ToBytes4(source.srcChain)
  }

  let resolvedDestChainKey: string | null = null
  let resolvedDestBridgeAddress: string | undefined
  if (destChain) {
    resolvedDestChainKey = getChainKeyByConfig(destChain) ?? null
    resolvedDestBridgeAddress = destChain.bridgeAddress
  }
  if (!resolvedDestChainKey && source?.destChain) {
    const bytes4 = bytes32ToBytes4(source.destChain)
    const entry = getBridgeChainEntryByBytes4(bytes4)
    if (entry) {
      const [destChainKey, destConfig] = entry
      resolvedDestChainKey = destChainKey
      resolvedDestBridgeAddress = destConfig.bridgeAddress
    }
  }

  return {
    id: hash,
    type: 'deposit',
    direction,
    sourceChain: resolvedSourceChainKey,
    destChain: resolvedDestChainKey ?? 'unknown',
    amount,
    status: 'confirmed',
    txHash: '',
    timestamp: Date.now(),
    xchainHashId: hash,
    lifecycle,
    depositNonce: source?.nonce !== undefined ? Number(source.nonce) : undefined,
    srcAccount: source?.srcAccount,
    destAccount: source?.destAccount,
    destToken: source?.token,
    destBridgeAddress: resolvedDestBridgeAddress,
    sourceChainIdBytes4: resolvedSourceBytes4,
  }
}

export default function TransferStatusPage() {
  const { xchainHashId } = useParams<{ xchainHashId: string }>()
  const { getTransferByXchainHashId, updateTransferRecord } = useTransferStore()
  const { lookup, source, sourceChain, dest, destChain, loading: lookupLoading } = useMultiChainLookup()

  const [transfer, setTransfer] = useState<TransferRecord | null>(null)

  // Try to find the transfer by hash in store; otherwise reset for lookup
  useEffect(() => {
    if (!xchainHashId) return
    const found = getTransferByXchainHashId(xchainHashId)
    setTransfer(found || null)
  }, [xchainHashId, getTransferByXchainHashId])

  // Lookup on chain for any valid hash (needed for synthetic transfer when not in store,
  // and for broken-transfer detection when transfer is in store)
  useEffect(() => {
    if (!xchainHashId) return
    if (!isValidXchainHashId(xchainHashId)) return
    lookup(normalizeXchainHashId(xchainHashId) as `0x${string}`)
  }, [xchainHashId, lookup])

  // Build transfer from lookup when we have chain data and no store match
  useEffect(() => {
    if (!xchainHashId) return
    if (getTransferByXchainHashId(xchainHashId)) return
    if (lookupLoading || (!source && !dest)) return
    const synthetic = buildTransferFromLookup(
      normalizeXchainHashId(xchainHashId),
      source,
      sourceChain,
      dest,
      destChain
    )
    setTransfer(synthetic)
  }, [xchainHashId, source, sourceChain, dest, destChain, lookupLoading, getTransferByXchainHashId])

  // Sync stale localStorage lifecycle with on-chain data.
  // When the on-chain lookup shows a more advanced lifecycle than what's stored,
  // update the stored record. Handles the case where the user left the page
  // before the TransferStatusPage polling could update the lifecycle.
  useEffect(() => {
    if (!xchainHashId || lookupLoading) return
    const stored = getTransferByXchainHashId(xchainHashId)
    if (!stored) return

    const onChainLifecycle: TransferLifecycle | undefined = dest?.executed
      ? 'executed'
      : dest?.approved
      ? 'approved'
      : dest
      ? 'hash-submitted'
      : undefined

    if (!onChainLifecycle) return

    const ORDER: TransferLifecycle[] = ['deposited', 'hash-submitted', 'approved', 'executed']
    const storedIdx = ORDER.indexOf(stored.lifecycle || 'deposited')
    const chainIdx = ORDER.indexOf(onChainLifecycle)

    if (chainIdx > storedIdx) {
      updateTransferRecord(stored.id, { lifecycle: onChainLifecycle })
      setTransfer((prev) => prev ? { ...prev, lifecycle: onChainLifecycle } : null)
    }
  }, [xchainHashId, dest, lookupLoading, getTransferByXchainHashId, updateTransferRecord])

  // Listen for store updates
  useEffect(() => {
    const handler = () => {
      if (!xchainHashId) return
      const found = getTransferByXchainHashId(xchainHashId)
      if (found) setTransfer(found)
    }
    window.addEventListener('cl8y-transfer-updated', handler)
    window.addEventListener('cl8y-transfer-recorded', handler)
    return () => {
      window.removeEventListener('cl8y-transfer-updated', handler)
      window.removeEventListener('cl8y-transfer-recorded', handler)
    }
  }, [xchainHashId, getTransferByXchainHashId])

  // Play success sound when transfer completes (only on transition, not on page load)
  const prevLifecycleRef = useRef<TransferLifecycle | undefined>(undefined)
  useEffect(() => {
    const current = transfer?.lifecycle
    if (current === 'executed' && prevLifecycleRef.current !== undefined && prevLifecycleRef.current !== 'executed') {
      sounds.playSuccess()
    }
    prevLifecycleRef.current = current
  }, [transfer?.lifecycle])

  // --- Terra nonce resolution ---
  // For terra-to-evm transfers that lack a depositNonce, parse it from LCD.
  // Uses state (not ref) so the user can retry if the LCD was unreachable.
  const [nonceStatus, setNonceStatus] = useState<NonceResolutionStatus>('idle')
  const nonceResolving = useRef(false)

  const retryNonceResolution = useCallback(() => {
    nonceResolving.current = false
    setNonceStatus('idle')
  }, [])

  useEffect(() => {
    if (!transfer) return
    if (transfer.direction !== 'terra-to-evm') return
    if (transfer.depositNonce !== undefined) {
      if (nonceStatus !== 'resolved') setNonceStatus('resolved')
      return
    }
    if (nonceStatus !== 'idle') return
    if (nonceResolving.current) return

    nonceResolving.current = true
    setNonceStatus('resolving')

    console.info(`${LOG} Starting nonce resolution for tx ${transfer.txHash}`)

    parseTerraLockReceipt(transfer.txHash).then((parsed) => {
      if (!parsed || parsed.nonce === undefined) {
        console.warn(`${LOG} Nonce resolution failed for tx ${transfer.txHash} (all retries exhausted)`)
        setNonceStatus('failed')
        return
      }

      // Compute the transfer hash now that we have the nonce
      let computedHash: string | undefined
      try {
        const tier = DEFAULT_NETWORK as NetworkTier
        const chains = BRIDGE_CHAINS[tier]
        const destChainConfig = chains[transfer.destChain]

        if (destChainConfig?.bytes4ChainId) {
          const srcChainB32 = chainIdToBytes32(2) // Terra = chain ID 2
          const destChainIdNum = parseInt(destChainConfig.bytes4ChainId.slice(2), 16)
          const destChainB32 = chainIdToBytes32(destChainIdNum)

          // Encode accounts -- they may already be bytes32 (66 chars) or raw addresses
          let srcAccB32: string
          if (transfer.srcAccount?.startsWith('terra')) {
            srcAccB32 = terraAddressToBytes32(transfer.srcAccount)
          } else if (transfer.srcAccount?.startsWith('0x') && transfer.srcAccount.length === 66) {
            srcAccB32 = transfer.srcAccount
          } else {
            srcAccB32 = '0x' + '0'.repeat(64)
          }

          let destAccB32: string
          if (transfer.destAccount?.startsWith('0x') && transfer.destAccount.length === 66) {
            destAccB32 = transfer.destAccount
          } else if (transfer.destAccount?.startsWith('0x') && transfer.destAccount.length === 42) {
            destAccB32 = evmAddressToBytes32(transfer.destAccount as `0x${string}`)
          } else {
            destAccB32 = '0x' + '0'.repeat(64)
          }

          let tokenB32: string
          if (transfer.destToken?.startsWith('0x') && transfer.destToken.length === 66) {
            tokenB32 = transfer.destToken
          } else if (transfer.destToken?.startsWith('0x') && transfer.destToken.length === 42) {
            tokenB32 = evmAddressToBytes32(transfer.destToken as `0x${string}`)
          } else {
            tokenB32 = '0x' + '0'.repeat(64)
          }

          computedHash = computeXchainHashId(
            srcChainB32 as `0x${string}`,
            destChainB32 as `0x${string}`,
            srcAccB32 as `0x${string}`,
            destAccB32 as `0x${string}`,
            tokenB32 as `0x${string}`,
            BigInt(parsed.amount || transfer.amount || '0'),
            BigInt(parsed.nonce)
          )
        }
      } catch (err) {
        console.warn(`${LOG} Failed to compute transfer hash:`, err)
      }

      console.info(
        `${LOG} Nonce resolved: nonce=${parsed.nonce}, netAmount=${parsed.amount}, ` +
        `hash=${computedHash?.slice(0, 18) ?? 'none'}...`
      )

      // Update transfer record with resolved nonce and net amount.
      // CRITICAL: The Terra bridge deducts a fee and uses the NET amount in the deposit hash.
      updateTransferRecord(transfer.id, {
        depositNonce: parsed.nonce,
        ...(parsed.amount ? { amount: parsed.amount } : {}),
        ...(computedHash ? { xchainHashId: computedHash } : {}),
      })
      setNonceStatus('resolved')
    }).catch((err) => {
      console.warn(`${LOG} Terra nonce parsing failed:`, err)
      setNonceStatus('failed')
    })
  }, [transfer, updateTransferRecord, nonceStatus])

  // --- Auto-submit orchestration ---
  const {
    phase: autoPhase,
    error: autoError,
    blockReason: autoBlockReason,
    canAutoSubmit,
    triggerSubmit,
    resetForRetry,
  } = useAutoWithdrawSubmit(transfer)

  // Detect: hash-submitted in localStorage but not on destination chain (tx reverted/lost).
  // Suppress when auto-submit is actively processing (submit just succeeded, polling will catch up).
  const autoSubmitActive =
    autoPhase === 'submitting-hash' ||
    autoPhase === 'waiting-approval' ||
    autoPhase === 'waiting-execution' ||
    autoPhase === 'switching-chain' ||
    autoPhase === 'complete'
  const hashSubmittedButMissing =
    transfer?.lifecycle === 'hash-submitted' &&
    !lookupLoading &&
    source != null &&
    dest == null &&
    !autoSubmitActive

  const [retryingHash, setRetryingHash] = useState(false)

  const handleRetryHashSubmission = useCallback(() => {
    if (!transfer) return
    setRetryingHash(true)
    resetForRetry()
    updateTransferRecord(transfer.id, { lifecycle: 'deposited' })
    setTransfer((prev) => prev ? { ...prev, lifecycle: 'deposited' } : null)
    // Re-trigger on-chain lookup after a short delay to pick up the new submission
    setTimeout(() => {
      setRetryingHash(false)
      if (xchainHashId && isValidXchainHashId(xchainHashId)) {
        lookup(normalizeXchainHashId(xchainHashId) as `0x${string}`)
      }
    }, 3000)
  }, [transfer, resetForRetry, updateTransferRecord, xchainHashId, lookup])

  // --- Broken transfer detection (dest exists, source null → wrong chain submitted) ---
  const normalizedHash = xchainHashId && isValidXchainHashId(xchainHashId)
    ? (normalizeXchainHashId(xchainHashId) as `0x${string}`)
    : undefined
  const { isBroken, fix, loading: fixLoading, error: fixError, retry: retryFixDetection } = useBrokenTransferFix(
    normalizedHash,
    source,
    dest,
    destChain
  )

  const { submitOnEvm, submitOnTerra } = useWithdrawSubmit()
  const { address: evmAddress, chain: evmChain } = useAccount()
  const { switchChainAsync } = useSwitchChain()
  const { connected: isTerraConnected } = useWallet()
  const [fixSubmitting, setFixSubmitting] = useState(false)
  const [fixSubmitError, setFixSubmitError] = useState<string | null>(null)

  const handleFixTransfer = useCallback(async () => {
    if (!fix || !transfer) return
    setFixSubmitting(true)
    setFixSubmitError(null)

    try {
      const { fixParams, correctHash, correctDestChain } = fix

      if (fixParams.destType === 'evm') {
        if (typeof correctDestChain.chainId !== 'number') {
          setFixSubmitError('Cannot determine destination chain ID for fix')
          setFixSubmitting(false)
          return
        }
        const destChainId = correctDestChain.chainId
        if (evmChain?.id !== destChainId) {
          await switchChainAsync({ chainId: destChainId as Parameters<typeof switchChainAsync>[0]['chainId'] })
        }

        const { bytes32ToAddress } = await import('../services/evm/tokenRegistry')
        const token: `0x${string}` =
          typeof fixParams.token === 'string' && fixParams.token.length === 66
            ? bytes32ToAddress(fixParams.token as `0x${string}`)
            : ('0x0000000000000000000000000000000000000000' as `0x${string}`)

        const txHash = await submitOnEvm({
          bridgeAddress: correctDestChain.bridgeAddress as `0x${string}`,
          srcChain: fixParams.srcChainBytes4,
          srcAccount: fixParams.srcAccount,
          destAccount: fixParams.destAccount,
          token,
          amount: fixParams.amount,
          nonce: fixParams.nonce,
        })

        if (txHash && transfer.id) {
          const updates = {
            xchainHashId: correctHash,
            withdrawSubmitTxHash: txHash,
            lifecycle: 'hash-submitted' as const,
            sourceChain: fix.wrongChainKey,
            destChain: fix.correctDestChainKey,
            direction: (fix.correctDestChain.type === 'cosmos' ? 'evm-to-terra' : 'evm-to-evm') as TransferRecord['direction'],
          }
          updateTransferRecord(transfer.id, updates)
          setTransfer((prev) => (prev ? { ...prev, ...updates } : null))
        }
      } else {
        // Terra destination
        const srcChainBytes4 = hexToUint8Array(fixParams.srcChainBytes4)
        const srcAccountBytes32 = hexToUint8Array(fixParams.srcAccount)

        const terraTxHash = await submitOnTerra({
          bridgeAddress: correctDestChain.bridgeAddress!,
          srcChainBytes4,
          srcAccountBytes32,
          token: fixParams.token as string,
          recipient: fixParams.terraRecipient || '',
          amount: fixParams.amount.toString(),
          nonce: Number(fixParams.nonce),
        })

        if (terraTxHash && transfer.id) {
          const updates = {
            xchainHashId: correctHash,
            withdrawSubmitTxHash: terraTxHash,
            lifecycle: 'hash-submitted' as const,
            sourceChain: fix.wrongChainKey,
            destChain: fix.correctDestChainKey,
            direction: 'evm-to-terra' as TransferRecord['direction'],
          }
          updateTransferRecord(transfer.id, updates)
          setTransfer((prev) => (prev ? { ...prev, ...updates } : null))
        }
      }
    } catch (err) {
      setFixSubmitError(err instanceof Error ? err.message : 'Fix failed')
    } finally {
      setFixSubmitting(false)
    }
  }, [fix, transfer, submitOnEvm, submitOnTerra, evmChain, switchChainAsync, updateTransferRecord])

  const currentStepIdx = useMemo(
    () => getStepIndex(transfer?.lifecycle),
    [transfer?.lifecycle]
  )

  const isFailed = transfer?.lifecycle === 'failed'

  const cancelWindowRemaining = useApprovalCountdown(
    transfer?.xchainHashId as `0x${string}` | undefined,
    transfer?.destChain,
    transfer?.lifecycle === 'approved'
  )

  const { data: bridgeConfigs } = useBridgeConfig()
  const destChainCancelWindow = transfer?.destChain
    ? bridgeConfigs?.find((c) => c.chainId === transfer.destChain)?.cancelWindowSeconds ?? null
    : null

  if (!transfer) {
    const isValidHash = xchainHashId && isValidXchainHashId(xchainHashId)
    const isLookingUp = isValidHash && lookupLoading

    return (
      <div className="mx-auto max-w-xl">
        <div className="shell-panel-strong text-center py-12 space-y-4">
          <h2 className="text-lg font-semibold text-white">
            {isLookingUp ? 'Looking up transfer...' : 'Transfer Not Found'}
          </h2>
          <p className="text-sm text-gray-400">
            {isLookingUp
              ? 'Querying chains for this hash.'
              : xchainHashId
              ? (
                <>
                  No transfer found with hash: <code className="text-xs break-all">{xchainHashId}</code>
                </>
              )
              : 'No hash provided.'}
          </p>
          {!isLookingUp && (
            <Link to="/" className="btn-primary inline-flex">
              Back to Bridge
            </Link>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-xl space-y-4">
      {/* Stepper */}
      <div className="shell-panel-strong relative overflow-hidden">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-8 top-2 h-24 rounded-[20px] theme-hero-glow blur-2xl"
        />
        <div className="relative z-10">
          <div className="mb-4 flex items-center justify-between gap-3">
            <h2 className="text-xl font-semibold uppercase tracking-[0.08em] text-white">Transfer Status</h2>
            <span className="border-2 border-white/35 bg-[#111111] px-2 py-1 font-mono text-[10px] font-semibold uppercase tracking-wide text-gray-300 shadow-[2px_2px_0_#000]">
              Step {Math.min(currentStepIdx + 1, STEPS.length)}/{STEPS.length}
            </span>
          </div>

          {isFailed && (
            <div className="mb-4 border-2 border-red-700 bg-[#221313] p-3 shadow-[3px_3px_0_#000]">
              <p className="text-red-300 text-xs font-semibold uppercase tracking-wide">
                Transfer Failed
              </p>
              <p className="text-red-400/80 text-xs mt-1">
                An error occurred during the transfer. You can retry from the step that failed.
              </p>
            </div>
          )}

          <div className="space-y-2">
            {STEPS.map((step, idx) => (
              <StepIndicator
                key={step.key}
                step={step}
                currentIdx={currentStepIdx}
                idx={idx}
                isFailed={isFailed}
              />
            ))}
          </div>

          {/* Active step message */}
          {!isFailed && transfer.lifecycle === 'deposited' && autoPhase === 'error' && (
            <div className="mt-2 border-2 border-red-700 bg-[#221313] p-3 shadow-[3px_3px_0_#000]">
              <p className="text-red-300 text-xs font-semibold uppercase tracking-wide">
                Hash Submission Failed
              </p>
              <p className="text-red-400/80 text-xs mt-1">
                {autoError || 'The withdrawSubmit transaction was rejected.'}
              </p>
              <p className="text-red-400/60 text-xs mt-1">
                This usually means the XChain Hash ID is invalid or was computed with incorrect parameters.
                The hash may need to be recomputed from the original deposit receipt.
              </p>
              <div className="mt-2 flex flex-wrap gap-2">
                {canAutoSubmit && (
                  <button
                    type="button"
                    onClick={() => triggerSubmit()}
                    className="btn-primary text-xs"
                  >
                    Retry Submit
                  </button>
                )}
                <Link to="/" className="btn-muted text-xs px-3 py-1">
                  New Transfer
                </Link>
              </div>
            </div>
          )}

          {!isFailed && transfer.lifecycle === 'deposited' && autoPhase !== 'error' && (
            <>
              {/* Nonce resolution failure banner */}
              {nonceStatus === 'failed' && (
                <div className="mt-2 border-2 border-red-700 bg-[#221313] p-3 shadow-[3px_3px_0_#000]">
                  <p className="text-red-300 text-xs font-semibold uppercase tracking-wide">
                    Nonce Resolution Failed
                  </p>
                  <p className="text-red-400/80 text-xs mt-1">
                    Could not extract the deposit nonce from the Terra LCD.
                    The LCD may be unreachable or the transaction is not yet indexed.
                  </p>
                  <button
                    type="button"
                    onClick={retryNonceResolution}
                    className="btn-primary mt-2 text-xs"
                  >
                    Retry Nonce Resolution
                  </button>
                </div>
              )}

              {/* Active step messages when nonce resolution hasn't failed */}
              {nonceStatus !== 'failed' && (
                <div className="mt-2 border-2 border-yellow-700 bg-[#222012] p-3 shadow-[3px_3px_0_#000]">
                  <p className="text-yellow-300 text-xs font-semibold uppercase tracking-wide">
                    {nonceStatus === 'resolving'
                      ? 'Resolving Deposit Nonce...'
                      : autoPhase === 'switching-chain'
                      ? 'Switching Chain...'
                      : autoPhase === 'submitting-hash'
                      ? 'Submitting Hash...'
                      : autoBlockReason === 'missing-nonce'
                      ? 'Waiting for Nonce Resolution'
                      : autoBlockReason === 'wallet-disconnected'
                      ? 'Wallet Not Connected'
                      : 'Waiting for Hash Submission'}
                  </p>
                  <p className="text-yellow-400/70 text-xs mt-1">
                    {nonceStatus === 'resolving'
                      ? 'Querying the Terra LCD for deposit nonce and amount. This may take a few seconds...'
                      : autoPhase === 'switching-chain'
                      ? 'Please approve the chain switch in your wallet.'
                      : autoPhase === 'submitting-hash'
                      ? 'Sending withdrawSubmit transaction to the destination chain...'
                      : autoBlockReason === 'missing-nonce'
                      ? 'The deposit nonce has not been resolved yet. The Terra LCD may still be indexing this transaction.'
                      : autoBlockReason === 'wallet-disconnected'
                      ? (transfer.direction === 'evm-to-evm' || transfer.direction === 'terra-to-evm')
                        ? 'Connect your EVM wallet to auto-submit the withdrawal hash.'
                        : 'Connect your Terra wallet to auto-submit the withdrawal hash.'
                      : canAutoSubmit
                      ? 'Auto-submitting via your connected wallet...'
                      : 'Connect your destination wallet to auto-submit, or use the manual flow below.'}
                  </p>
                  {/* Manual retry button when wallet connected but auto-submit didn't fire */}
                  {autoPhase === 'manual-required' && canAutoSubmit && (
                    <button
                      type="button"
                      onClick={() => triggerSubmit()}
                      className="btn-primary mt-2 text-xs"
                    >
                      Retry Submit
                    </button>
                  )}
                </div>
              )}
            </>
          )}

          {!isFailed && transfer.lifecycle === 'hash-submitted' && (
            <>
              {/* Broken transfer: wrong chain submitted, offer fix */}
              {isBroken && fix && (
                <div className="mt-2 border-2 border-amber-700 bg-[#221a12] p-3 shadow-[3px_3px_0_#000]">
                  <p className="text-amber-300 text-xs font-semibold uppercase tracking-wide">
                    Hash Submitted on Wrong Chain
                  </p>
                  <p className="text-amber-400/80 text-xs mt-1">
                    The withdrawal was submitted on {fix.wrongChain.name} but should have been on {fix.correctDestChain.name}.
                    The operator cannot approve it. Submit on the correct chain to fix.
                  </p>
                  <div className="mt-2 flex flex-wrap gap-2 items-center">
                    <button
                      type="button"
                      onClick={handleFixTransfer}
                      disabled={fixSubmitting || (fix.fixParams.destType === 'evm' && !evmAddress) || (fix.fixParams.destType === 'cosmos' && !isTerraConnected)}
                      className="btn-primary text-xs"
                    >
                      {fixSubmitting ? 'Submitting…' : `Fix: Submit on ${fix.correctDestChain.name}`}
                    </button>
                    <button type="button" onClick={retryFixDetection} disabled={fixLoading} className="btn-muted text-xs">
                      Retry Detection
                    </button>
                    {fixSubmitError && (
                      <span className="text-red-400 text-xs">{fixSubmitError}</span>
                    )}
                  </div>
                  {(fix.fixParams.destType === 'evm' && !evmAddress) && (
                    <p className="text-amber-400/60 text-xs mt-2">Connect your EVM wallet to fix.</p>
                  )}
                  {(fix.fixParams.destType === 'cosmos' && !isTerraConnected) && (
                    <p className="text-amber-400/60 text-xs mt-2">Connect your Terra wallet to fix.</p>
                  )}
                </div>
              )}
              {(!isBroken || !fix) && hashSubmittedButMissing && (
                <div className="mt-2 border-2 border-red-700 bg-[#221313] p-3 shadow-[3px_3px_0_#000]">
                  <p className="text-red-300 text-xs font-semibold uppercase tracking-wide">
                    Hash Not Found on Destination
                  </p>
                  <p className="text-red-400/80 text-xs mt-1">
                    The deposit exists on the source chain, but the hash submission transaction
                    was not confirmed on the destination chain. The transaction may have reverted
                    or was lost when you navigated away.
                  </p>
                  <div className="mt-2 flex flex-wrap gap-2 items-center">
                    <button
                      type="button"
                      onClick={handleRetryHashSubmission}
                      disabled={retryingHash}
                      className="btn-primary text-xs"
                    >
                      {retryingHash ? 'Retrying…' : 'Retry Hash Submission'}
                    </button>
                  </div>
                </div>
              )}
              {(!isBroken || !fix) && !hashSubmittedButMissing && (
                <div className="mt-2 border-2 border-cyan-700 bg-[#121c22] p-3 shadow-[3px_3px_0_#000]">
                  <p className="text-blue-300 text-xs font-semibold uppercase tracking-wide">
                    Waiting for Operator Approval
                  </p>
                  <p className="text-blue-400/70 text-xs mt-1">
                    The operator is verifying your deposit on the source chain.
                    {destChainCancelWindow != null ? (
                      <> After approval, the cancel window is {formatCancelWindowRange(destChainCancelWindow)} before tokens are released.</>
                    ) : (
                      <> This usually takes a few minutes depending on the destination chain.</>
                    )}
                  </p>
                  {isBroken && fixLoading && (
                    <p className="text-amber-400/70 text-xs mt-1">Checking for fix option…</p>
                  )}
                  {isBroken && fixError && (
                    <p className="text-amber-400/70 text-xs mt-1">Could not find a fix: {fixError}</p>
                  )}
                </div>
              )}
            </>
          )}

          {!isFailed && transfer.lifecycle === 'approved' && (
            <div className="mt-2 border-2 border-cyan-700 bg-[#121c22] p-3 shadow-[3px_3px_0_#000]">
              <p className="text-blue-300 text-xs font-semibold uppercase tracking-wide">
                Cancel Window Active
              </p>
              <p className="text-blue-400/70 text-xs mt-1">
                Approved. Waiting for the cancel window to expire before tokens are released
                {destChainCancelWindow != null && (
                  <span className="ml-1">({formatCancelWindowRange(destChainCancelWindow)})</span>
                )}
                .
                {cancelWindowRemaining != null && cancelWindowRemaining > 0 ? (
                  <span className="ml-1 font-mono text-base font-semibold tabular-nums text-cyan-300">
                    {formatCountdownMmSs(cancelWindowRemaining)} remaining
                  </span>
                ) : cancelWindowRemaining != null && cancelWindowRemaining <= 0 ? (
                  <span className="ml-1 font-mono text-base font-semibold tabular-nums text-cyan-300">
                    Executing…
                  </span>
                ) : null}
              </p>
            </div>
          )}

          {transfer.lifecycle === 'executed' && (
            <div className="mt-2 border-2 border-white/35 bg-[#161616] p-3 shadow-[3px_3px_0_#000]">
              <div className="flex items-start gap-2">
                <span className="inline-flex h-7 w-7 shrink-0 items-center justify-center border-2 border-[#b8ff3d]/70 bg-[#2a3518] shadow-[1px_1px_0_#000]">
                  <img src="/assets/status-success.png" alt="" className="h-4 w-4 object-contain" aria-hidden />
                </span>
                <div>
                  <p className="text-[#b8ff3d] text-xs font-semibold uppercase tracking-wide">
                    Transfer Complete
                  </p>
                  <p className="text-gray-300 text-xs mt-1">
                    Tokens have been delivered to the recipient address.
                  </p>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Transfer details */}
      <div className="shell-panel-strong">
        <h3 className="mb-3 text-sm font-semibold text-white uppercase tracking-wide">Details</h3>
        <TransferDetails transfer={transfer} />
      </div>

      {/* Manual flow info */}
      {transfer.xchainHashId && transfer.lifecycle === 'deposited' && (
        <div className="shell-panel-strong">
          <h3 className="mb-2 text-sm font-semibold text-white uppercase tracking-wide">
            Manual Flow
          </h3>
          <p className="text-xs text-gray-400 mb-3">
            If auto-submit did not trigger, you can manually submit the hash on the destination
            chain. Use the{' '}
            <Link to={`/verify?hash=${encodeURIComponent(transfer.xchainHashId)}`} className="text-[#b8ff3d] hover:underline">
              Verify page
            </Link>{' '}
            to submit it.
          </p>
        </div>
      )}

      {/* Navigation */}
      <div className="flex gap-3">
        <Link to="/" className="btn-muted flex-1 justify-center py-2">
          Back to Bridge
        </Link>
        <Link to="/history" className="btn-muted flex-1 justify-center py-2">
          View History
        </Link>
        {transfer.xchainHashId && (
          <Link
            to={`/verify?hash=${encodeURIComponent(transfer.xchainHashId)}`}
            className="btn-muted flex-1 justify-center py-2"
          >
            Verify Hash
          </Link>
        )}
      </div>
    </div>
  )
}
