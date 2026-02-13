import { useState, useMemo, useEffect, useCallback } from 'react'
import { useAccount, usePublicClient } from 'wagmi'
import { useNavigate } from 'react-router-dom'
import { Address, getAddress } from 'viem'
import { useWallet } from '../../hooks/useWallet'
import { useTokenRegistry } from '../../hooks/useTokenRegistry'
import {
  useBridgeDeposit,
  computeTerraChainBytes4,
  computeEvmChainBytes4,
  encodeTerraAddress,
  encodeEvmAddress,
} from '../../hooks/useBridgeDeposit'
import { useTerraDeposit } from '../../hooks/useTerraDeposit'
import { useTransferStore } from '../../stores/transfer'
import { useUIStore } from '../../stores/ui'
import { getChainById } from '../../lib/chains'
import { getChainsForTransfer, BRIDGE_CHAINS, type NetworkTier } from '../../utils/bridgeChains'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'
import { parseDepositFromLogs } from '../../services/evm/depositReceipt'
import { getDestToken } from '../../services/evm/tokenRegistry'
import { computeTransferHash, chainIdToBytes32, evmAddressToBytes32, terraAddressToBytes32 } from '../../services/hashVerification'
import type { ChainInfo } from '../../lib/chains'
import type { TransferDirection } from '../../types/transfer'
import type { TokenOption } from './TokenSelect'
import { DEFAULT_NETWORK, BRIDGE_CONFIG, DECIMALS } from '../../utils/constants'
import { parseAmount, formatAmount } from '../../utils/format'
import { isValidAmount } from '../../utils/validation'
import { sounds } from '../../lib/sounds'
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

const LOCK_UNLOCK_ADDRESS = (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address

/** Build selectable token options from registry for the given direction */
function buildTransferTokens(
  registryTokens: { token: string; is_native: boolean; evm_token_address: string; terra_decimals: number; evm_decimals: number; enabled: boolean }[] | undefined,
  isSourceTerra: boolean,
  fallbackConfig: { address: Address; symbol: string; decimals: number } | undefined
): TokenOption[] {
  if (isSourceTerra) {
    // Terra source: show all enabled tokens (native + CW20).
    // Native tokens use deposit_native, CW20 tokens use deposit (send CW20 to bridge).
    // Both are registered in the Terra bridge token registry.
    const fromRegistry = (registryTokens ?? [])
      .filter((t) => t.enabled)
      .map((t) => ({
        id: t.token,
        symbol: getTokenDisplaySymbol(t.token),
        tokenId: t.token,
      }))
    if (fromRegistry.length > 0) return fromRegistry
    return [{ id: 'uluna', symbol: 'LUNC', tokenId: 'uluna' }]
  }
  // EVM source: show enabled tokens with evm_token_address
  const fromRegistry = (registryTokens ?? [])
    .filter((t) => t.enabled && t.evm_token_address)
    .map((t) => ({
      id: t.token,
      symbol: getTokenDisplaySymbol(t.token),
      tokenId: t.token,
    }))
  if (fromRegistry.length > 0) return fromRegistry
  if (fallbackConfig) {
    return [{ id: fallbackConfig.address, symbol: fallbackConfig.symbol, tokenId: fallbackConfig.address }]
  }
  return []
}

/**
 * Convert a bytes32 hex string (64 chars) or 20-byte address to a checksummed EVM address.
 * The Terra bridge stores EVM addresses left-padded to 32 bytes.
 * Invalid/short input returns zero address to avoid crashing on malformed registry data.
 */
function bytes32ToAddress(hex: string): Address {
  const clean = hex.replace(/^0x/, '').toLowerCase()
  if (!/^[0-9a-f]*$/.test(clean) || clean.length < 40) {
    return '0x0000000000000000000000000000000000000000' as Address
  }
  const addr = clean.length > 40 ? clean.slice(-40) : clean
  try {
    return getAddress(`0x${addr}`)
  } catch {
    return '0x0000000000000000000000000000000000000000' as Address
  }
}

/** Get EVM token config for deposit/balance from selected token id */
function getEvmTokenConfig(
  selectedTokenId: string,
  registryTokens: { token: string; evm_token_address: string; evm_decimals: number }[] | undefined,
  fallbackConfig: { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined
): { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined {
  const fromRegistry = registryTokens?.find((t) => t.token === selectedTokenId)
  if (fromRegistry?.evm_token_address) {
    return {
      address: bytes32ToAddress(fromRegistry.evm_token_address),
      lockUnlockAddress: LOCK_UNLOCK_ADDRESS,
      symbol: getTokenDisplaySymbol(fromRegistry.token),
      decimals: fromRegistry.evm_decimals,
    }
  }
  if (fallbackConfig && (selectedTokenId === fallbackConfig.address || selectedTokenId === 'uluna')) {
    return fallbackConfig
  }
  return undefined
}

/** Get Terra token decimals for selected token */
function getTerraTokenDecimals(
  selectedTokenId: string,
  registryTokens: { token: string; terra_decimals: number }[] | undefined
): number {
  const fromRegistry = registryTokens?.find((t) => t.token === selectedTokenId)
  return fromRegistry?.terra_decimals ?? DECIMALS.LUNC
}

export function TransferForm() {
  const { isConnected: isEvmConnected, address: evmAddress } = useAccount()
  const { connected: isTerraConnected, address: terraAddress, luncBalance, setShowWalletModal } = useWallet()
  const { data: registryTokens } = useTokenRegistry()
  const { setShowEvmWalletModal } = useUIStore()
  const { recordTransfer } = useTransferStore()
  const navigate = useNavigate()
  const publicClient = usePublicClient()

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
  const [selectedTokenId, setSelectedTokenId] = useState<string>('')
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

  const fallbackTokenConfig = TOKEN_CONFIGS[DEFAULT_NETWORK]
  const transferTokens = useMemo(
    () => buildTransferTokens(registryTokens, isSourceTerra, fallbackTokenConfig),
    [registryTokens, isSourceTerra, fallbackTokenConfig]
  )

  const evmTokenConfig = useMemo(
    () => getEvmTokenConfig(selectedTokenId, registryTokens, fallbackTokenConfig),
    [selectedTokenId, registryTokens, fallbackTokenConfig]
  )
  const tokenConfig = isSourceTerra ? undefined : evmTokenConfig
  const terraDecimals = useMemo(
    () => getTerraTokenDecimals(selectedTokenId, registryTokens),
    [selectedTokenId, registryTokens]
  )

  useEffect(() => {
    if (transferTokens.length === 0) return
    const validIds = transferTokens.map((t) => t.id)
    const currentValid = validIds.includes(selectedTokenId)
    if (!currentValid || !selectedTokenId) {
      setSelectedTokenId(transferTokens[0].id)
    }
  }, [transferTokens, selectedTokenId])

  // Derive source chain bridge address and native chain ID for multi-EVM support.
  // When the source is an EVM chain (e.g. anvil1), we must deposit on THAT chain's bridge,
  // not the primary bridge. We also need the native chain ID for auto-switching.
  const sourceBridgeConfig = useMemo(() => {
    const tier = DEFAULT_NETWORK as NetworkTier
    const config = BRIDGE_CHAINS[tier][sourceChain]
    if (!config || config.type !== 'evm') return undefined
    return {
      bridgeAddress: config.bridgeAddress as Address,
      nativeChainId: typeof config.chainId === 'number' ? config.chainId : undefined,
    }
  }, [sourceChain])

  const {
    deposit: evmDeposit,
    status: evmStatus,
    depositTxHash,
    error: evmError,
    reset: resetEvm,
    tokenBalance,
  } = useBridgeDeposit(
    tokenConfig
      ? {
          tokenAddress: tokenConfig.address,
          lockUnlockAddress: tokenConfig.lockUnlockAddress,
          bridgeAddress: sourceBridgeConfig?.bridgeAddress,
          sourceNativeChainId: sourceBridgeConfig?.nativeChainId,
        }
      : undefined
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

  const amountDecimals = isSourceTerra ? terraDecimals : (tokenConfig?.decimals ?? DECIMALS.LUNC)

  // EVM deposit success: parse receipt, compute transfer hash, store record, redirect
  useEffect(() => {
    if (evmStatus === 'success' && depositTxHash) {
      setTxHash(depositTxHash)

      // Try to parse the deposit receipt for nonce and other fields
      const parseAndRecord = async () => {
        let depositNonce: number | undefined
        let depositAmount: string = parseAmount(amount, amountDecimals)
        let tokenAddr: string | undefined = tokenConfig?.address
        let srcAccountHex: string | undefined
        let destAccountHex: string | undefined

        // Parse deposit event from tx receipt
        try {
          if (publicClient) {
            const receipt = await publicClient.getTransactionReceipt({ hash: depositTxHash as `0x${string}` })
            const parsed = parseDepositFromLogs(receipt.logs)
            if (parsed) {
              depositNonce = Number(parsed.nonce)
              depositAmount = parsed.amount.toString()
              tokenAddr = parsed.token
              srcAccountHex = parsed.srcAccount
              destAccountHex = parsed.destAccount
            }
          }
        } catch (err) {
          console.warn('[TransferForm] Failed to parse deposit receipt:', err)
        }

        // Compute transfer hash if we have the nonce
        let transferHash: string | undefined
        if (depositNonce !== undefined) {
          try {
            const tier = DEFAULT_NETWORK as NetworkTier
            const chains = BRIDGE_CHAINS[tier]
            const srcChainConfig = chains[sourceChain]
            const destChainConfig = chains[destChain]

            if (srcChainConfig?.bytes4ChainId && destChainConfig?.bytes4ChainId) {
              // Convert bytes4 hex to chain ID number for chainIdToBytes32
              const srcChainIdNum = parseInt(srcChainConfig.bytes4ChainId.slice(2), 16)
              const destChainIdNum = parseInt(destChainConfig.bytes4ChainId.slice(2), 16)

              const srcChainB32 = chainIdToBytes32(srcChainIdNum)
              const destChainB32 = chainIdToBytes32(destChainIdNum)
              const tokenB32 = tokenAddr ? evmAddressToBytes32(tokenAddr as `0x${string}`) : '0x' + '0'.repeat(64)
              const srcAccB32 = srcAccountHex || (evmAddress ? evmAddressToBytes32(evmAddress as `0x${string}`) : '0x' + '0'.repeat(64))
              const destAccB32 = destAccountHex || '0x' + '0'.repeat(64)

              transferHash = computeTransferHash(
                srcChainB32 as `0x${string}`,
                destChainB32 as `0x${string}`,
                srcAccB32 as `0x${string}`,
                destAccB32 as `0x${string}`,
                tokenB32 as `0x${string}`,
                BigInt(depositAmount),
                BigInt(depositNonce)
              )
            }
          } catch (err) {
            console.warn('[TransferForm] Failed to compute transfer hash:', err)
          }
        }

        // Get source chain bytes4 for the record
        const tier = DEFAULT_NETWORK as NetworkTier
        const chains = BRIDGE_CHAINS[tier]
        const srcChainConfig = chains[sourceChain]
        const destChainConfig = chains[destChain]

        // V2 fix: query the correct destination token from TokenRegistry
        let destTokenBytes32: string | undefined
        if (publicClient && tokenAddr && srcChainConfig?.bridgeAddress && destChainConfig?.bytes4ChainId) {
          try {
            const dt = await getDestToken(
              publicClient,
              srcChainConfig.bridgeAddress as Address,
              tokenAddr as Address,
              destChainConfig.bytes4ChainId as `0x${string}`
            )
            if (dt) destTokenBytes32 = dt
          } catch (err) {
            console.warn('[TransferForm] Failed to query destToken from registry:', err)
          }
        }

        const id = recordTransfer({
          type: 'deposit',
          direction,
          sourceChain,
          destChain,
          amount: depositAmount,
          status: 'confirmed',
          txHash: depositTxHash,
          lifecycle: 'deposited',
          transferHash,
          depositNonce,
          srcAccount: srcAccountHex || (evmAddress ? evmAddressToBytes32(evmAddress as `0x${string}`) : undefined),
          destAccount: destAccountHex || recipientAddr,
          token: tokenAddr || selectedTokenId,
          srcDecimals: amountDecimals,
          destToken: destTokenBytes32 || tokenAddr,
          destBridgeAddress: destChainConfig?.bridgeAddress,
          sourceChainIdBytes4: srcChainConfig?.bytes4ChainId,
        })

        // Redirect to transfer status page
        if (transferHash) {
          navigate(`/transfer/${transferHash}`)
        } else if (id) {
          navigate(`/transfer/${id}`)
        }
      }

      parseAndRecord()
      resetEvm()
    } else if (evmStatus === 'error' && evmError) {
      setError(evmError)
      resetEvm()
    }
  }, [evmStatus, depositTxHash, evmError, resetEvm, sourceChain, destChain, amount, amountDecimals, recordTransfer, direction, navigate, publicClient, evmAddress, tokenConfig, recipientAddr, selectedTokenId])

  // Terra deposit success: store record and redirect
  useEffect(() => {
    if (terraStatus === 'success' && terraTxHash) {
      setTxHash(terraTxHash)

      // Get chain configs for the record
      const tier = DEFAULT_NETWORK as NetworkTier
      const chains = BRIDGE_CHAINS[tier]
      const destChainConfig = chains[destChain]

      // For Terra, nonce parsing is async via LCD and may not be immediate.
      // Store the record now and parse nonce later on the status page.
      // V2 fix: encode srcAccount as bech32-decoded bytes32
      let srcAccountHex: string | undefined
      if (terraAddress) {
        try {
          srcAccountHex = terraAddressToBytes32(terraAddress)
        } catch {
          srcAccountHex = terraAddress // fallback: store raw bech32
        }
      }

      // V2 fix: encode destAccount as bytes32 for EVM recipient
      const destAccountHex = recipientAddr.startsWith('0x')
        ? evmAddressToBytes32(recipientAddr as `0x${string}`)
        : recipientAddr

      // Resolve the destination EVM token address from the Terra bridge's token registry.
      // Each Terra token has an evm_token_address (bytes32) that maps to the ERC20 on the
      // destination EVM chain. The auto-submit hook needs this to call withdrawSubmit.
      const selectedRegistryToken = registryTokens?.find((t) => t.token === (selectedTokenId || 'uluna'))
      let destTokenB32: string | undefined
      if (selectedRegistryToken?.evm_token_address) {
        const raw = selectedRegistryToken.evm_token_address
        destTokenB32 = raw.startsWith('0x') ? raw : '0x' + raw
      }

      const id = recordTransfer({
        type: 'withdrawal',
        direction: 'terra-to-evm',
        sourceChain: sourceChain.includes('terra') || sourceChain.includes('local') ? sourceChain : 'localterra',
        destChain,
        amount: parseAmount(amount, terraDecimals),
        status: 'confirmed',
        txHash: terraTxHash,
        lifecycle: 'deposited',
        srcAccount: srcAccountHex,
        destAccount: destAccountHex,
        token: selectedTokenId || 'uluna',
        destToken: destTokenB32,
        srcDecimals: terraDecimals,
        destBridgeAddress: destChainConfig?.bridgeAddress,
        sourceChainIdBytes4: '0x00000002', // Terra chain ID in bridge
      })

      navigate(`/transfer/${id}`)
      resetTerra()
    } else if (terraStatus === 'error' && terraError) {
      setError(terraError)
      resetTerra()
    }
  }, [terraStatus, terraTxHash, terraError, resetTerra, navigate, destChain, amount, terraDecimals, recordTransfer, terraAddress, recipientAddr, selectedTokenId, registryTokens])

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

    // Look up destination chain's V2 bytes4 chain ID from BRIDGE_CHAINS config.
    // The V2 protocol uses predetermined chain IDs (e.g. anvil=1, terra=2, anvil1=3),
    // NOT native chain IDs (e.g. 31337, 31338).
    const tier = DEFAULT_NETWORK as NetworkTier
    const destConfig = BRIDGE_CHAINS[tier][destChain]

    if (direction === 'terra-to-evm') {
      // Terra → EVM: deposit_native on Terra bridge (V2)
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      // Use V2 chain ID from config (e.g. 1 for anvil, 3 for anvil1), NOT native chain ID
      const v2Bytes4 = destConfig?.bytes4ChainId
      const destChainId = v2Bytes4 ? parseInt(v2Bytes4, 16) : getChainIdNumeric(destChain, allChains)
      const amountMicro = parseAmount(amount, terraDecimals)
      await terraLock({ amountMicro, destChainId, recipientEvm: recipientAddr })
    } else if (direction === 'evm-to-terra') {
      // EVM → Terra: depositERC20 on Bridge (V2) with bytes4 Terra chain ID
      if (!recipientAddr || !recipientAddr.startsWith('terra1')) {
        setError('Please provide a Terra recipient address or connect your Terra wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      const destChainBytes4 = computeTerraChainBytes4()
      const destAccount = encodeTerraAddress(recipientAddr)
      await evmDeposit(amount, destChainBytes4, destAccount, tokenConfig.decimals)
    } else {
      // EVM → EVM: depositERC20 on Bridge (V2) with bytes4 dest chain ID
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      // Use V2 bytes4 chain ID from config, NOT native chain ID
      const destChainBytes4 = (destConfig?.bytes4ChainId as Hex) || computeEvmChainBytes4(getChainIdNumeric(destChain, allChains))
      const destAccount = encodeEvmAddress(recipientAddr)
      await evmDeposit(amount, destChainBytes4, destAccount, tokenConfig.decimals)
    }
  }

  const balanceDisplay = isSourceTerra
    ? formatAmount(luncBalance, terraDecimals)
    : tokenBalance !== undefined && tokenConfig
    ? formatAmount(tokenBalance.toString(), tokenConfig.decimals)
    : undefined

  const selectedSymbol =
    transferTokens.find((t) => t.id === selectedTokenId)?.symbol ?? (isSourceTerra ? 'LUNC' : tokenConfig?.symbol ?? 'LUNC')
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
          <p className="text-green-400/60 text-xs mt-1">Redirecting to transfer status...</p>
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
        balanceLabel={selectedSymbol}
      />
      <AmountInput
        value={amount}
        onChange={setAmount}
        onMax={
          isSourceTerra
            ? () => setAmount(formatAmount(luncBalance, terraDecimals))
            : tokenBalance !== undefined && tokenConfig
            ? () => setAmount(formatAmount(tokenBalance.toString(), tokenConfig.decimals))
            : undefined
        }
        tokens={transferTokens}
        selectedTokenId={selectedTokenId}
        onTokenChange={setSelectedTokenId}
        symbol={selectedSymbol}
      />
      <SwapDirectionButton onClick={handleSwap} disabled={isSwapDisabled} />
      <DestChainSelector chains={destChains} value={destChain} onChange={setDestChain} />
      <RecipientInput
        value={recipient}
        onChange={setRecipient}
        direction={direction}
        onAutofill={() => {
          if (isDestEvm) {
            if (evmAddress) setRecipient(evmAddress)
            else setShowEvmWalletModal(true)
          } else {
            if (terraAddress) setRecipient(terraAddress)
            else setShowWalletModal(true)
          }
        }}
      />
      <FeeBreakdown receiveAmount={receiveAmount} symbol={selectedSymbol} />

      <button
        type="submit"
        data-testid="submit-transfer"
        disabled={!isWalletConnected || !amount || !isValidAmount(amount) || isSubmitting}
        onClick={() => sounds.playButtonPress()}
        className="btn-primary btn-cta w-full justify-center py-3 disabled:bg-gray-700 disabled:text-gray-400 disabled:shadow-none disabled:translate-x-0 disabled:translate-y-0 disabled:cursor-not-allowed"
      >
        {buttonText}
      </button>
    </form>
  )
}
