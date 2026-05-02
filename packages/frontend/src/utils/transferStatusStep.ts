import type { TransferLifecycle } from '../types/transfer'
import type { AutoSubmitPhase } from '../hooks/useAutoWithdrawSubmit'

export type ComputeTransferStepIdxArgs = {
  transferLifecycle: TransferLifecycle | undefined
  destIsSolana: boolean
  effectiveCancelWindowRemaining: number | null | undefined
  autoPhase: AutoSubmitPhase
  retryingHash: boolean
  source: unknown | null
  dest: unknown | null
  /** Background multi-chain lookup may toggle; must not snap the stepper off Submit Hash (GL-131). */
  lookupLoading: boolean
}

/**
 * Active step index for the Transfer Status vertical stepper.
 * Extracted for unit tests — see GL-131 (stepper must not flicker when `lookupLoading` toggles during polls).
 */
export function computeTransferStepIdx(args: ComputeTransferStepIdxArgs): number {
  const {
    transferLifecycle,
    destIsSolana,
    effectiveCancelWindowRemaining,
    autoPhase,
    retryingHash,
    source,
    dest,
  } = args

  const baseIdx = getStepIndex(transferLifecycle, destIsSolana, effectiveCancelWindowRemaining)

  if (transferLifecycle === 'deposited' && autoPhase === 'error') return 1
  if (retryingHash) return 1
  if (autoPhase === 'submitting-hash' && source != null && transferLifecycle === 'hash-submitted') return 1
  if (transferLifecycle === 'hash-submitted' && autoPhase === 'error') return 1
  if (autoPhase === 'submitting-hash' && source != null && transferLifecycle === 'deposited') return 1
  // Do not gate on lookupLoading: polling sets loading=true ~every POLLING_INTERVAL, which must not
  // revert the highlight from Submit Hash back to Deposit (GL-131).
  if (transferLifecycle === 'deposited' && source != null && dest == null) {
    return 1
  }
  return baseIdx
}

/** Same semantics as TransferStatusPage.getStepIndex (Solana inserts an execute step). */
function getStepIndex(
  lifecycle: TransferLifecycle | undefined,
  destIsSolana: boolean,
  cancelWindowRemaining: number | null | undefined,
): number {
  const n = destIsSolana ? 5 : 4
  if (!lifecycle || lifecycle === 'failed') return 0
  if (lifecycle === 'executed') return n
  if (lifecycle === 'deposited') return 0
  if (lifecycle === 'hash-submitted') return 2
  if (lifecycle === 'approved') {
    if (destIsSolana) {
      if (cancelWindowRemaining != null && cancelWindowRemaining > 0) return 2
      return 3
    }
    return 3
  }
  return 0
}
