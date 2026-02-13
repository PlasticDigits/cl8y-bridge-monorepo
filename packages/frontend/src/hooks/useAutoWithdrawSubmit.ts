/**
 * useAutoWithdrawSubmit Hook
 *
 * Automatically orchestrates step 2 of the V2 bridge protocol:
 * after a deposit is confirmed, this hook detects the deposit,
 * computes the transfer hash, and auto-calls withdrawSubmit on
 * the destination chain via the user's connected wallet.
 *
 * For EVM -> EVM transfers, handles chain switching first.
 *
 * Also polls the destination chain for approval and execution status.
 *
 * V2 fixes:
 * - Uses transfer.sourceChainIdBytes4 directly (no parseInt of chain name)
 * - Terra srcAccount is bech32-decoded to bytes32 via terraAddressToBytes32
 * - destToken is used from the TransferRecord (set correctly by TransferForm)
 * - Falls back to querying TokenRegistry if destToken is missing
 */

import { useEffect, useRef, useCallback, useState } from 'react'
import { useAccount, usePublicClient, useSwitchChain } from 'wagmi'
import type { Address, Hex } from 'viem'
import { useWallet } from './useWallet'
import { useWithdrawSubmit } from './useWithdrawSubmit'
import { useTransferStore } from '../stores/transfer'
import { BRIDGE_WITHDRAW_VIEW_ABI } from '../services/evm/withdrawSubmit'
import { queryTerraPendingWithdraw } from '../services/terraBridgeQueries'
import { getDestToken, bytes32ToAddress } from '../services/evm/tokenRegistry'
import {
  chainIdToBytes4,
  evmAddressToBytes32Array,
  hexToUint8Array,
} from '../services/terra/withdrawSubmit'
import { terraAddressToBytes32 } from '../services/hashVerification'
import { CONTRACTS, DEFAULT_NETWORK, POLLING_INTERVAL } from '../utils/constants'
import { BRIDGE_CHAINS } from '../utils/bridgeChains'
import type { TransferRecord } from '../types/transfer'

export type AutoSubmitPhase =
  | 'idle'
  | 'ready'               // Transfer loaded, ready to auto-submit
  | 'switching-chain'      // EVM->EVM: switching to dest chain
  | 'submitting-hash'      // Sending withdrawSubmit tx
  | 'waiting-approval'     // Polling for operator approval
  | 'waiting-execution'    // Polling for execution after approval
  | 'complete'
  | 'error'
  | 'manual-required'      // Wallets not connected or auto-submit failed

export function useAutoWithdrawSubmit(transfer: TransferRecord | null) {
  const { address: evmAddress, chain: evmChain } = useAccount()
  const { connected: isTerraConnected } = useWallet()
  const { switchChainAsync } = useSwitchChain()
  const publicClient = usePublicClient()
  const { submitOnEvm, submitOnTerra } = useWithdrawSubmit()
  const { updateTransferRecord } = useTransferStore()

  const [phase, setPhase] = useState<AutoSubmitPhase>('idle')
  const [error, setError] = useState<string | null>(null)
  const submittedRef = useRef(false)
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Cleanup polling on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) clearInterval(pollingRef.current)
    }
  }, [])

  // Determine if auto-submit is possible
  const canAutoSubmit = useCallback((): boolean => {
    if (!transfer || transfer.lifecycle !== 'deposited') return false
    if (!transfer.depositNonce && transfer.depositNonce !== 0) return false

    if (transfer.direction === 'terra-to-evm' || transfer.direction === 'evm-to-evm') {
      // Destination is EVM - need EVM wallet
      return !!evmAddress
    }
    if (transfer.direction === 'evm-to-terra') {
      // Destination is Terra - need Terra wallet
      return isTerraConnected
    }
    return false
  }, [transfer, evmAddress, isTerraConnected])

  // Get destination chain config
  const getDestChainConfig = useCallback(() => {
    if (!transfer) return null
    const tier = DEFAULT_NETWORK as 'local' | 'testnet' | 'mainnet'
    const chains = BRIDGE_CHAINS[tier]
    return chains[transfer.destChain] || null
  }, [transfer])

  // Auto-submit effect
  useEffect(() => {
    if (!transfer || transfer.lifecycle !== 'deposited' || submittedRef.current) {
      return
    }

    if (!canAutoSubmit()) {
      setPhase('manual-required')
      return
    }

    setPhase('ready')
  }, [transfer, canAutoSubmit])

  /**
   * Resolve the destination token address for EVM withdrawSubmit.
   * Uses transfer.destToken if available, otherwise queries TokenRegistry.
   */
  const resolveDestToken = useCallback(async (): Promise<Address> => {
    // If destToken is already set and looks like an address (not bytes32), use it directly
    if (transfer?.destToken && transfer.destToken !== '0x' + '0'.repeat(64)) {
      const dt = transfer.destToken
      // If it's a bytes32, extract the address
      if (dt.length === 66) {
        return bytes32ToAddress(dt as Hex)
      }
      // It's already an address
      return dt as Address
    }

    // Fallback: query the destination chain's TokenRegistry
    // We need a public client for the source chain to query token mappings
    if (publicClient && transfer?.token) {
      const tier = DEFAULT_NETWORK as 'local' | 'testnet' | 'mainnet'
      const chains = BRIDGE_CHAINS[tier]
      const srcChainConfig = chains[transfer.sourceChain]
      const destChainConfig = chains[transfer.destChain]

      if (srcChainConfig?.bridgeAddress && destChainConfig?.bytes4ChainId) {
        const destTokenB32 = await getDestToken(
          publicClient,
          srcChainConfig.bridgeAddress as Address,
          transfer.token as Address,
          destChainConfig.bytes4ChainId as Hex
        )
        if (destTokenB32) {
          return bytes32ToAddress(destTokenB32)
        }
      }
    }

    // Last resort: zero address (will likely fail, but lets the tx attempt proceed)
    return '0x0000000000000000000000000000000000000000' as Address
  }, [transfer, publicClient])

  /**
   * Trigger the withdrawSubmit call.
   * Called automatically when phase is 'ready', or manually by user.
   */
  const triggerSubmit = useCallback(async () => {
    if (!transfer || submittedRef.current) return
    submittedRef.current = true

    try {
      const destChainConfig = getDestChainConfig()

      if (transfer.direction === 'terra-to-evm' || transfer.direction === 'evm-to-evm') {
        // Destination is EVM
        const destChainId = (typeof destChainConfig?.chainId === 'number'
          ? destChainConfig.chainId
          : 31337) as number

        // For EVM->EVM, may need to switch chains
        if (transfer.direction === 'evm-to-evm' && evmChain?.id !== destChainId) {
          setPhase('switching-chain')
          try {
            await switchChainAsync({ chainId: destChainId as Parameters<typeof switchChainAsync>[0]['chainId'] })
          } catch (err) {
            setPhase('error')
            setError(`Failed to switch to destination chain: ${err instanceof Error ? err.message : String(err)}`)
            submittedRef.current = false
            return
          }
        }

        setPhase('submitting-hash')

        // Build params for EVM withdrawSubmit
        const bridgeAddress = (destChainConfig?.bridgeAddress ||
          CONTRACTS[DEFAULT_NETWORK].evmBridge) as Address

        // V2 fix: use sourceChainIdBytes4 directly instead of parsing chain name as int
        const srcChainBytes4 = (transfer.sourceChainIdBytes4 || '0x00000002') as Hex

        // V2 fix: if srcAccount is a Terra bech32 address, convert it properly
        let srcAccountHex = (transfer.srcAccount || '0x' + '0'.repeat(64)) as Hex
        if (transfer.srcAccount?.startsWith('terra1')) {
          srcAccountHex = terraAddressToBytes32(transfer.srcAccount)
        }

        // V2 fix: resolve the correct destination token
        const destTokenAddress = await resolveDestToken()

        const txHash = await submitOnEvm({
          bridgeAddress,
          srcChain: srcChainBytes4,
          srcAccount: srcAccountHex,
          destAccount: (transfer.destAccount || '0x' + '0'.repeat(64)) as Hex,
          token: destTokenAddress,
          amount: BigInt(transfer.amount || '0'),
          nonce: BigInt(transfer.depositNonce || 0),
          srcDecimals: transfer.srcDecimals || 6,
        })

        if (txHash) {
          updateTransferRecord(transfer.id, {
            lifecycle: 'hash-submitted',
            withdrawSubmitTxHash: txHash,
          })
          setPhase('waiting-approval')
          startPolling()
        } else {
          setPhase('error')
          setError('WithdrawSubmit transaction failed')
          submittedRef.current = false
        }
      } else if (transfer.direction === 'evm-to-terra') {
        // Destination is Terra
        setPhase('submitting-hash')

        const bridgeAddress = CONTRACTS[DEFAULT_NETWORK].terraBridge

        // V2 fix: use sourceChainIdBytes4 to derive the bytes4 for src chain
        let srcChainBytes4Data: Uint8Array
        if (transfer.sourceChainIdBytes4) {
          srcChainBytes4Data = hexToUint8Array(transfer.sourceChainIdBytes4)
        } else {
          // Fallback: try to determine from source chain config
          const tier = DEFAULT_NETWORK as 'local' | 'testnet' | 'mainnet'
          const chains = BRIDGE_CHAINS[tier]
          const srcConfig = chains[transfer.sourceChain]
          if (srcConfig?.bytes4ChainId) {
            srcChainBytes4Data = hexToUint8Array(srcConfig.bytes4ChainId)
          } else if (typeof srcConfig?.chainId === 'number') {
            srcChainBytes4Data = chainIdToBytes4(srcConfig.chainId)
          } else {
            srcChainBytes4Data = chainIdToBytes4(31337)
          }
        }

        // srcAccount: should be the EVM depositor address as bytes32
        const srcAccountBytes32 = transfer.srcAccount
          ? hexToUint8Array(transfer.srcAccount)
          : evmAddressToBytes32Array(evmAddress || '0x0000000000000000000000000000000000000000')

        const terraTxHash = await submitOnTerra({
          bridgeAddress,
          srcChainBytes4: srcChainBytes4Data,
          srcAccountBytes32,
          token: transfer.token || 'uluna',
          recipient: transfer.destAccount || '',
          amount: transfer.amount || '0',
          nonce: transfer.depositNonce || 0,
        })

        if (terraTxHash) {
          updateTransferRecord(transfer.id, {
            lifecycle: 'hash-submitted',
            withdrawSubmitTxHash: terraTxHash,
          })
          setPhase('waiting-approval')
          startPolling()
        } else {
          setPhase('error')
          setError('WithdrawSubmit transaction failed')
          submittedRef.current = false
        }
      }
    } catch (err) {
      setPhase('error')
      setError(err instanceof Error ? err.message : 'Auto-submit failed')
      submittedRef.current = false
    }
  }, [transfer, evmAddress, evmChain, switchChainAsync, submitOnEvm, submitOnTerra, updateTransferRecord, getDestChainConfig, resolveDestToken])

  /**
   * Poll destination chain for approval and execution.
   * Supports both EVM (RPC) and Terra (LCD) destinations.
   */
  const startPolling = useCallback(() => {
    if (pollingRef.current) clearInterval(pollingRef.current)

    pollingRef.current = setInterval(async () => {
      if (!transfer?.transferHash) return

      const destChainConfig = getDestChainConfig()
      if (!destChainConfig || !destChainConfig.bridgeAddress) return

      try {
        if (destChainConfig.type === 'evm') {
          // EVM destination: query via RPC
          if (!publicClient) return

          const bridgeAddress = destChainConfig.bridgeAddress as Address
          const result = await publicClient.readContract({
            address: bridgeAddress,
            abi: BRIDGE_WITHDRAW_VIEW_ABI,
            functionName: 'getPendingWithdraw',
            args: [transfer.transferHash as Hex],
          }) as { submittedAt: bigint; approvedAt: bigint; approved: boolean; executed: boolean }

          if (result.executed) {
            if (transfer.lifecycle !== 'executed') {
              updateTransferRecord(transfer.id, { lifecycle: 'executed' })
              setPhase('complete')
            }
            if (pollingRef.current) {
              clearInterval(pollingRef.current)
              pollingRef.current = null
            }
          } else if (result.approved || result.approvedAt > 0n) {
            if (transfer.lifecycle === 'hash-submitted') {
              updateTransferRecord(transfer.id, { lifecycle: 'approved' })
              setPhase('waiting-execution')
            }
          }
        } else if (destChainConfig.type === 'cosmos') {
          // Terra destination: query via LCD
          const lcdUrls = destChainConfig.lcdFallbacks || (destChainConfig.lcdUrl ? [destChainConfig.lcdUrl] : [])
          if (lcdUrls.length === 0) return

          const result = await queryTerraPendingWithdraw(
            lcdUrls,
            destChainConfig.bridgeAddress,
            transfer.transferHash as Hex,
            destChainConfig
          )

          if (result?.executed) {
            if (transfer.lifecycle !== 'executed') {
              updateTransferRecord(transfer.id, { lifecycle: 'executed' })
              setPhase('complete')
            }
            if (pollingRef.current) {
              clearInterval(pollingRef.current)
              pollingRef.current = null
            }
          } else if (result?.approved) {
            if (transfer.lifecycle === 'hash-submitted') {
              updateTransferRecord(transfer.id, { lifecycle: 'approved' })
              setPhase('waiting-execution')
            }
          }
        }
      } catch {
        // Polling error, continue on next interval
      }
    }, POLLING_INTERVAL)
  }, [transfer, publicClient, updateTransferRecord, getDestChainConfig])

  // Auto-trigger submit when ready
  useEffect(() => {
    if (phase === 'ready' && !submittedRef.current) {
      triggerSubmit()
    }
  }, [phase, triggerSubmit])

  // Start polling if transfer is already hash-submitted or approved
  useEffect(() => {
    if (transfer?.lifecycle === 'hash-submitted' || transfer?.lifecycle === 'approved') {
      startPolling()
    }
    return () => {
      if (pollingRef.current) clearInterval(pollingRef.current)
    }
  }, [transfer?.lifecycle, startPolling])

  return {
    phase,
    error,
    canAutoSubmit: canAutoSubmit(),
    triggerSubmit,
  }
}
