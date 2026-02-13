/**
 * Transfer Status Page
 *
 * Route: /transfer/:transferHash
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
import { useTransferStore } from '../stores/transfer'
import { useAutoWithdrawSubmit } from '../hooks/useAutoWithdrawSubmit'
import { parseTerraLockReceipt } from '../services/terra/depositReceipt'
import { computeTransferHash, chainIdToBytes32, evmAddressToBytes32, terraAddressToBytes32 } from '../services/hashVerification'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'
import type { TransferRecord, TransferLifecycle } from '../types/transfer'

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
  return idx >= 0 ? idx : 0
}

function StepIndicator({ step, currentIdx, idx }: { step: typeof STEPS[number]; currentIdx: number; idx: number }) {
  const isDone = idx < currentIdx
  const isActive = idx === currentIdx
  const isFuture = idx > currentIdx

  return (
    <div className="flex items-start gap-3">
      {/* Circle */}
      <div className="flex flex-col items-center">
        <div
          className={`flex h-8 w-8 shrink-0 items-center justify-center border-2 font-mono text-xs font-bold
            ${isDone ? 'border-[#b8ff3d] bg-[#b8ff3d]/20 text-[#b8ff3d]' : ''}
            ${isActive ? 'border-[#b8ff3d] bg-[#b8ff3d] text-black animate-pulse' : ''}
            ${isFuture ? 'border-gray-600 bg-transparent text-gray-600' : ''}
          `}
        >
          {isDone ? '\u2713' : idx + 1}
        </div>
        {idx < STEPS.length - 1 && (
          <div className={`h-8 w-0.5 ${isDone ? 'bg-[#b8ff3d]/40' : 'bg-gray-700'}`} />
        )}
      </div>
      {/* Text */}
      <div className="pb-6">
        <p
          className={`text-sm font-semibold uppercase tracking-wide ${
            isDone ? 'text-[#b8ff3d]' : isActive ? 'text-white' : 'text-gray-500'
          }`}
        >
          {step.label}
        </p>
        <p className={`text-xs ${isDone || isActive ? 'text-gray-300' : 'text-gray-600'}`}>
          {step.description}
        </p>
      </div>
    </div>
  )
}

function TransferDetails({ transfer }: { transfer: TransferRecord }) {
  const [copied, setCopied] = useState(false)

  const copyHash = useCallback(() => {
    if (transfer.transferHash) {
      navigator.clipboard.writeText(transfer.transferHash)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }, [transfer.transferHash])

  return (
    <div className="space-y-3 text-sm">
      {/* Transfer hash */}
      {transfer.transferHash && (
        <div>
          <p className="text-xs uppercase tracking-wide text-gray-400 mb-1">Transfer Hash</p>
          <div className="flex items-center gap-2">
            <code className="flex-1 truncate font-mono text-xs text-gray-200 bg-black/40 border border-gray-700 px-2 py-1">
              {transfer.transferHash}
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

export default function TransferStatusPage() {
  const { transferHash } = useParams<{ transferHash: string }>()
  const { getTransferByHash, getTransferById, updateTransferRecord } = useTransferStore()

  // Try to find the transfer by hash or by ID
  const [transfer, setTransfer] = useState<TransferRecord | null>(null)

  useEffect(() => {
    if (!transferHash) return
    let found = getTransferByHash(transferHash)
    if (!found) found = getTransferById(transferHash)
    setTransfer(found)
  }, [transferHash, getTransferByHash, getTransferById])

  // Listen for updates
  useEffect(() => {
    const handler = () => {
      if (!transferHash) return
      let found = getTransferByHash(transferHash)
      if (!found) found = getTransferById(transferHash)
      setTransfer(found)
    }
    window.addEventListener('cl8y-transfer-updated', handler)
    window.addEventListener('cl8y-transfer-recorded', handler)
    return () => {
      window.removeEventListener('cl8y-transfer-updated', handler)
      window.removeEventListener('cl8y-transfer-recorded', handler)
    }
  }, [transferHash, getTransferByHash, getTransferById])

  // --- Terra nonce resolution ---
  // For terra-to-evm transfers that lack a depositNonce, parse it from LCD
  const terraNonceResolved = useRef(false)
  useEffect(() => {
    if (!transfer) return
    if (transfer.direction !== 'terra-to-evm') return
    if (transfer.depositNonce !== undefined) return
    if (terraNonceResolved.current) return

    terraNonceResolved.current = true

    parseTerraLockReceipt(transfer.txHash).then((parsed) => {
      if (!parsed || parsed.nonce === undefined) {
        console.warn('[TransferStatusPage] Could not parse Terra lock nonce from tx:', transfer.txHash)
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

          // Encode accounts
          const srcAccB32 = transfer.srcAccount?.startsWith('terra')
            ? terraAddressToBytes32(transfer.srcAccount)
            : '0x' + '0'.repeat(64)
          const destAccB32 = transfer.destAccount?.startsWith('0x')
            ? evmAddressToBytes32(transfer.destAccount as `0x${string}`)
            : '0x' + '0'.repeat(64)

          const tokenB32 = transfer.destToken?.startsWith('0x')
            ? evmAddressToBytes32(transfer.destToken as `0x${string}`)
            : '0x' + '0'.repeat(64)

          computedHash = computeTransferHash(
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
        console.warn('[TransferStatusPage] Failed to compute transfer hash:', err)
      }

      updateTransferRecord(transfer.id, {
        depositNonce: parsed.nonce,
        ...(computedHash ? { transferHash: computedHash } : {}),
      })
    }).catch((err) => {
      console.warn('[TransferStatusPage] Terra nonce parsing failed:', err)
    })
  }, [transfer, updateTransferRecord])

  // --- Auto-submit orchestration ---
  const {
    phase: autoPhase,
    error: autoError,
    canAutoSubmit,
    triggerSubmit,
  } = useAutoWithdrawSubmit(transfer)

  const currentStepIdx = useMemo(
    () => getStepIndex(transfer?.lifecycle),
    [transfer?.lifecycle]
  )

  const isFailed = transfer?.lifecycle === 'failed'

  if (!transfer) {
    return (
      <div className="mx-auto max-w-xl">
        <div className="shell-panel-strong text-center py-12 space-y-4">
          <h2 className="text-lg font-semibold text-white">Transfer Not Found</h2>
          <p className="text-sm text-gray-400">
            No transfer found with hash: <code className="text-xs break-all">{transferHash}</code>
          </p>
          <Link to="/" className="btn-primary inline-flex">
            Back to Bridge
          </Link>
        </div>
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-xl space-y-4">
      {/* Stepper */}
      <div className="shell-panel-strong">
        <h2 className="mb-4 text-lg font-semibold text-white">Transfer Status</h2>

        {isFailed && (
          <div className="mb-4 bg-red-900/30 border-2 border-red-700 p-3">
            <p className="text-red-300 text-xs font-semibold uppercase tracking-wide">
              Transfer Failed
            </p>
            <p className="text-red-400/80 text-xs mt-1">
              An error occurred during the transfer. You can retry from the step that failed.
            </p>
          </div>
        )}

        <div className="pl-1">
          {STEPS.map((step, idx) => (
            <StepIndicator
              key={step.key}
              step={step}
              currentIdx={currentStepIdx}
              idx={idx}
            />
          ))}
        </div>

        {/* Active step message */}
        {!isFailed && transfer.lifecycle === 'deposited' && (
          <div className="mt-2 bg-yellow-900/20 border border-yellow-700/50 p-3">
            <p className="text-yellow-300 text-xs font-semibold uppercase tracking-wide">
              {autoPhase === 'switching-chain'
                ? 'Switching Chain...'
                : autoPhase === 'submitting-hash'
                ? 'Submitting Hash...'
                : autoPhase === 'error'
                ? 'Auto-Submit Failed'
                : 'Waiting for Hash Submission'}
            </p>
            <p className="text-yellow-400/70 text-xs mt-1">
              {autoPhase === 'switching-chain'
                ? 'Please approve the chain switch in your wallet.'
                : autoPhase === 'submitting-hash'
                ? 'Sending withdrawSubmit transaction to the destination chain...'
                : autoPhase === 'error'
                ? autoError || 'An error occurred. You can retry or use the manual flow.'
                : canAutoSubmit
                ? 'Auto-submitting via your connected wallet...'
                : transfer.direction === 'evm-to-evm'
                ? 'Switch to the destination chain and submit the withdrawal hash.'
                : 'Connect your destination wallet to auto-submit, or use the manual flow below.'}
            </p>
            {/* Manual retry button when auto-submit fails or manual is required */}
            {(autoPhase === 'error' || autoPhase === 'manual-required') && canAutoSubmit && (
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

        {!isFailed && transfer.lifecycle === 'hash-submitted' && (
          <div className="mt-2 bg-blue-900/20 border border-blue-700/50 p-3">
            <p className="text-blue-300 text-xs font-semibold uppercase tracking-wide">
              Waiting for Operator Approval
            </p>
            <p className="text-blue-400/70 text-xs mt-1">
              The operator is verifying your deposit on the source chain. This usually takes 10-30 seconds.
            </p>
          </div>
        )}

        {!isFailed && transfer.lifecycle === 'approved' && (
          <div className="mt-2 bg-blue-900/20 border border-blue-700/50 p-3">
            <p className="text-blue-300 text-xs font-semibold uppercase tracking-wide">
              Cancel Window Active
            </p>
            <p className="text-blue-400/70 text-xs mt-1">
              Approved. Waiting for the cancel window to expire before tokens are released.
            </p>
          </div>
        )}

        {transfer.lifecycle === 'executed' && (
          <div className="mt-2 bg-green-900/20 border border-green-700/50 p-3">
            <p className="text-green-300 text-xs font-semibold uppercase tracking-wide">
              Transfer Complete
            </p>
            <p className="text-green-400/70 text-xs mt-1">
              Tokens have been delivered to the recipient address.
            </p>
          </div>
        )}
      </div>

      {/* Transfer details */}
      <div className="shell-panel-strong">
        <h3 className="mb-3 text-sm font-semibold text-white uppercase tracking-wide">Details</h3>
        <TransferDetails transfer={transfer} />
      </div>

      {/* Manual flow info */}
      {transfer.transferHash && transfer.lifecycle === 'deposited' && (
        <div className="shell-panel-strong">
          <h3 className="mb-2 text-sm font-semibold text-white uppercase tracking-wide">
            Manual Flow
          </h3>
          <p className="text-xs text-gray-400 mb-3">
            If auto-submit did not trigger, you can manually submit the hash on the destination
            chain. Copy the transfer hash above and use the{' '}
            <Link to="/verify" className="text-[#b8ff3d] hover:underline">
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
        {transfer.transferHash && (
          <Link
            to={`/verify`}
            className="btn-muted flex-1 justify-center py-2"
          >
            Verify Hash
          </Link>
        )}
      </div>
    </div>
  )
}
