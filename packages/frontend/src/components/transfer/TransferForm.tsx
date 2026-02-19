import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
import { useAccount, usePublicClient } from 'wagmi'
import { useNavigate } from 'react-router-dom'
import { Address, getAddress, type Hex } from 'viem'
import { useWallet } from '../../hooks/useWallet'
import { useTokenRegistry } from '../../hooks/useTokenRegistry'
import { useTokenList } from '../../hooks/useTokenList'
import { useTokenDestMapping } from '../../hooks/useTokenDestMapping'
import { useSourceChainTokenMappings } from '../../hooks/useSourceChainTokenMappings'
import { useBridgeConfig, useTokenDetails } from '../../hooks/useBridgeConfig'
import { useTerraTokenDisplayInfo, useEvmTokenDisplayInfo } from '../../hooks/useTokenDisplayInfo'
import {
  useBridgeDeposit,
  computeTerraChainBytes4,
  encodeTerraAddress,
  encodeEvmAddress,
} from '../../hooks/useBridgeDeposit'
import { useTerraDeposit } from '../../hooks/useTerraDeposit'
import { useTransferStore } from '../../stores/transfer'
import { useUIStore } from '../../stores/ui'
import { getChainById } from '../../lib/chains'
import { getChainsForTransfer, BRIDGE_CHAINS, type NetworkTier } from '../../utils/bridgeChains'
import { useDiscoveredChains } from '../../hooks/useDiscoveredChains'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'
import { getTokenFromList, type TokenlistData } from '../../services/tokenlist'
import { shortenAddress } from '../../utils/shortenAddress'
import { getTokenExplorerUrl } from '../../utils/format'
import { parseDepositFromLogs } from '../../services/evm/depositReceipt'
import { parseTerraLockReceipt } from '../../services/terra/depositReceipt'
import { getDestToken } from '../../services/evm/tokenRegistry'
import { getEvmClient } from '../../services/evmClient'
import { computeXchainHashId, chainIdToBytes32, evmAddressToBytes32, terraAddressToBytes32 } from '../../services/hashVerification'
import type { ChainInfo } from '../../lib/chains'
import type { TransferDirection } from '../../types/transfer'
import type { TokenOption } from './TokenSelect'
import { DEFAULT_NETWORK, BRIDGE_CONFIG, DECIMALS } from '../../utils/constants'
import { parseAmount, formatAmount, formatCompact } from '../../utils/format'
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
  fallbackConfig: { address: Address; symbol: string; decimals: number } | undefined,
  tokenlist: TokenlistData | null,
  /** When source is EVM: per-chain token address from Terra token_dest_mapping; falls back to registry evm_token_address when missing */
  sourceChainMappings?: Record<string, string>
): TokenOption[] {
  const symbolFromList = (token: string) =>
    tokenlist ? getTokenFromList(tokenlist, token)?.symbol : null
  if (isSourceTerra) {
    const fromRegistry = (registryTokens ?? [])
      .filter((t) => t.enabled)
      .map((t) => ({
        id: t.token,
        symbol: symbolFromList(t.token) ?? getTokenDisplaySymbol(t.token),
        tokenId: t.token,
      }))
    if (fromRegistry.length > 0) return fromRegistry
    return [{ id: 'uluna', symbol: 'LUNC', tokenId: 'uluna' }]
  }
  // EVM source: show all enabled tokens with evm_token_address.
  // Use per-chain token_dest_mapping when available, else fall back to registry evm_token_address.
  const baseRegistry = (registryTokens ?? []).filter((t) => t.enabled && t.evm_token_address)
  const fromRegistry = baseRegistry.map((t) => ({
    id: t.token,
    symbol: symbolFromList(t.token) ?? getTokenDisplaySymbol(t.token),
    tokenId: t.token,
    evmTokenAddress:
      sourceChainMappings?.[t.token] ?? bytes32ToAddress(t.evm_token_address),
  }))
  if (fromRegistry.length > 0) return fromRegistry
  if (fallbackConfig && !sourceChainMappings) {
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
  fallbackConfig: { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined,
  /** When source is EVM: use per-chain address from token_dest_mapping */
  sourceChainMappings?: Record<string, string>
): { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined {
  const fromRegistry = registryTokens?.find((t) => t.token === selectedTokenId)
  const mappedAddr = sourceChainMappings?.[selectedTokenId]
  const evmAddr = mappedAddr ?? fromRegistry?.evm_token_address
  if (evmAddr) {
    return {
      address: (mappedAddr ?? bytes32ToAddress(evmAddr)) as Address,
      lockUnlockAddress: LOCK_UNLOCK_ADDRESS,
      symbol: getTokenDisplaySymbol(fromRegistry?.token ?? selectedTokenId),
      decimals: fromRegistry?.evm_decimals ?? 18,
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
  const { data: registryTokens, isLoading: isRegistryLoading } = useTokenRegistry()
  const { data: tokenlist } = useTokenList()
  const { setShowEvmWalletModal } = useUIStore()
  const { recordTransfer } = useTransferStore()
  const navigate = useNavigate()
  const publicClient = usePublicClient()

  const { chains: allChains, isLoading: isChainsLoading } = useDiscoveredChains()

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

  // --- Frozen chain state for deposit recording ---
  // When a deposit is initiated, we freeze the source/dest chain values so that
  // if the user changes the chain selectors (e.g. via the swap button) while the
  // deposit tx is in flight, the TransferRecord still captures the correct chains.
  // Without this, swapping chains during a pending deposit causes the record to be
  // stored with swapped source/dest, leading to withdrawSubmit on the wrong chain.
  const frozenChainsRef = useRef<{
    sourceChain: string
    destChain: string
    direction: TransferDirection
    sourceChainIdBytes4: string | undefined
  } | null>(null)

  // Compute valid destination chains based on source selection
  const destChains = useMemo(() => getValidDestChains(allChains, sourceChain), [allChains, sourceChain])

  // When source changes, ensure dest is still valid
  useEffect(() => {
    const validIds = destChains.map((c) => c.id)
    if (!validIds.includes(destChain) && validIds.length > 0 && validIds[0] !== undefined) {
      setDestChain(validIds[0])
    }
  }, [destChains, destChain])

  const isSourceTerra = direction === 'terra-to-evm'
  const isDestEvm = direction === 'terra-to-evm' || direction === 'evm-to-evm'

  const sourceChainConfig = useMemo(
    () => BRIDGE_CHAINS[DEFAULT_NETWORK as NetworkTier]?.[sourceChain],
    [sourceChain]
  )
  const { data: bridgeConfigData } = useBridgeConfig()
  const isSourceEvm = sourceChainConfig?.type === 'evm'
  const sourceChainBytes4 = sourceChainConfig?.bytes4ChainId
  const { mappings: sourceChainMappings, isLoading: isSourceMappingsLoading } = useSourceChainTokenMappings(
    registryTokens,
    sourceChainBytes4,
    !!isSourceEvm && !!sourceChainBytes4
  )

  const fallbackTokenConfig = TOKEN_CONFIGS[DEFAULT_NETWORK]
  const transferTokens = useMemo(
    () =>
      buildTransferTokens(
        registryTokens,
        isSourceTerra,
        fallbackTokenConfig,
        tokenlist ?? null,
        isSourceEvm ? sourceChainMappings : undefined
      ),
    [registryTokens, isSourceTerra, fallbackTokenConfig, tokenlist, isSourceEvm, sourceChainMappings]
  )

  const evmTokenConfig = useMemo(
    () =>
      getEvmTokenConfig(
        selectedTokenId,
        registryTokens,
        fallbackTokenConfig,
        isSourceEvm ? sourceChainMappings : undefined
      ),
    [selectedTokenId, registryTokens, fallbackTokenConfig, isSourceEvm, sourceChainMappings]
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
    if ((!currentValid || !selectedTokenId) && transferTokens[0]) {
      setSelectedTokenId(transferTokens[0].id)
    }
  }, [transferTokens, selectedTokenId])

  const destChainConfig = useMemo(
    () => BRIDGE_CHAINS[DEFAULT_NETWORK as NetworkTier]?.[destChain],
    [destChain]
  )
  const destChainBytes4 = destChainConfig?.bytes4ChainId
  const isDestEvmChain = destChainConfig?.type === 'evm'
  const { data: tokenDestMappingAddr } = useTokenDestMapping(
    selectedTokenId || undefined,
    destChainBytes4,
    !!destChainBytes4 && !!isDestEvmChain
  )

  const destTokenAddr = useMemo(() => {
    if (destChainConfig?.type !== 'evm') return ''
    if (tokenDestMappingAddr) return tokenDestMappingAddr
    const reg = registryTokens?.find((t) => t.token === selectedTokenId)
    return reg?.evm_token_address ? bytes32ToAddress(reg.evm_token_address) : ''
  }, [destChainConfig?.type, tokenDestMappingAddr, registryTokens, selectedTokenId])

  // Destination chain limits (max transfer, withdraw rate limit) cap the MAX amount
  const destChainUnifiedConfig = useMemo(
    () => bridgeConfigData?.find((c) => c.chainId === destChain) ?? null,
    [bridgeConfigData, destChain]
  )
  const destTokenIdForDetails =
    destChainConfig?.type === 'cosmos' ? selectedTokenId : destTokenAddr || null
  const { data: destTokenDetails } = useTokenDetails(
    destChainUnifiedConfig,
    destTokenIdForDetails,
    !!destChainUnifiedConfig && !!destTokenIdForDetails
  )

  const terraDisplay = useTerraTokenDisplayInfo(selectedTokenId || undefined)
  const evmSourceDisplay = useEvmTokenDisplayInfo(
    tokenConfig?.address,
    sourceChainConfig?.type === 'evm' ? sourceChainConfig.rpcUrl : undefined,
    !isSourceTerra && !!tokenConfig
  )
  const evmDestDisplay = useEvmTokenDisplayInfo(
    destTokenAddr,
    destChainConfig?.type === 'evm' ? destChainConfig.rpcUrl : undefined,
    !!isDestEvmChain && !!destTokenAddr
  )

  // Derive source chain bridge address and native chain ID for multi-EVM support.
  // When the source is an EVM chain (e.g. anvil1), we must deposit on THAT chain's bridge,
  // not a fixed base chain bridge. We also need the native chain ID for auto-switching.
  const sourceBridgeConfig = useMemo(() => {
    const tier = DEFAULT_NETWORK as NetworkTier
    const config = BRIDGE_CHAINS[tier][sourceChain]
    if (!config || config.type !== 'evm') return undefined
    return {
      bridgeAddress: config.bridgeAddress as Address,
      nativeChainId: typeof config.chainId === 'number' ? config.chainId : undefined,
      rpcUrl: config.rpcUrl,
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
          sourceRpcUrl: sourceBridgeConfig?.rpcUrl,
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
  const destDecimals = isDestEvmChain
    ? (registryTokens?.find((t) => t.token === selectedTokenId)?.evm_decimals ?? 18)
    : terraDecimals

  const toSourceUnits = useCallback(
    (destBaseUnits: bigint) => {
      if (amountDecimals >= destDecimals) {
        const exp = amountDecimals - destDecimals
        return destBaseUnits * BigInt(10 ** exp)
      }
      const exp = destDecimals - amountDecimals
      return destBaseUnits / BigInt(10 ** exp)
    },
    [amountDecimals, destDecimals]
  )

  const { displayMaxLabel, displayMinLabel } = useMemo(() => {
    const srcDecimals = amountDecimals
    let balanceStr: string | undefined
    if (isSourceTerra) {
      balanceStr = luncBalance
    } else if (tokenBalance !== undefined && tokenConfig) {
      balanceStr = tokenBalance.toString()
    }
    const balance = balanceStr ? BigInt(balanceStr) : 0n

    let maxTransferInSrc: bigint | null = null
    if (destTokenDetails?.maxTransfer) {
      const raw = BigInt(destTokenDetails.maxTransfer)
      if (raw > 0n) maxTransferInSrc = toSourceUnits(raw)
    }
    let bridgeRemainingInSrc: bigint | null = null
    if (destTokenDetails?.withdrawRateLimit?.remainingAmount) {
      const raw = BigInt(destTokenDetails.withdrawRateLimit.remainingAmount)
      if (raw > 0n) bridgeRemainingInSrc = toSourceUnits(raw)
    }

    let effectiveMax = balance
    if (maxTransferInSrc != null && maxTransferInSrc > 0n)
      effectiveMax = effectiveMax < maxTransferInSrc ? effectiveMax : maxTransferInSrc
    if (bridgeRemainingInSrc != null && bridgeRemainingInSrc > 0n)
      effectiveMax = effectiveMax < bridgeRemainingInSrc ? effectiveMax : bridgeRemainingInSrc

    let effectiveMin: bigint | null = null
    if (destTokenDetails?.minTransfer) {
      const raw = BigInt(destTokenDetails.minTransfer)
      if (raw > 0n) effectiveMin = toSourceUnits(raw)
    }

    return {
      displayMaxLabel:
        effectiveMax > 0n ? formatCompact(effectiveMax.toString(), srcDecimals) : undefined,
      displayMinLabel:
        effectiveMin != null && effectiveMin > 0n
          ? formatCompact(effectiveMin.toString(), srcDecimals)
          : undefined,
    }
  }, [
    isSourceTerra,
    luncBalance,
    tokenBalance,
    tokenConfig,
    destTokenDetails?.maxTransfer,
    destTokenDetails?.minTransfer,
    destTokenDetails?.withdrawRateLimit?.remainingAmount,
    amountDecimals,
    toSourceUnits,
  ])

  const handleMax = useCallback(() => {
    const srcDecimals = amountDecimals
    let balanceStr: string
    if (isSourceTerra) {
      balanceStr = luncBalance
    } else if (tokenBalance !== undefined && tokenConfig) {
      balanceStr = tokenBalance.toString()
    } else {
      return
    }
    const balance = BigInt(balanceStr)

    let maxTransferInSrc: bigint | null = null
    if (destTokenDetails?.maxTransfer) {
      const raw = BigInt(destTokenDetails.maxTransfer)
      if (raw > 0n) maxTransferInSrc = toSourceUnits(raw)
    }
    let bridgeRemainingInSrc: bigint | null = null
    if (destTokenDetails?.withdrawRateLimit?.remainingAmount) {
      const raw = BigInt(destTokenDetails.withdrawRateLimit.remainingAmount)
      if (raw > 0n) bridgeRemainingInSrc = toSourceUnits(raw)
    }

    let effectiveMax = balance
    if (maxTransferInSrc != null && maxTransferInSrc > 0n)
      effectiveMax = effectiveMax < maxTransferInSrc ? effectiveMax : maxTransferInSrc
    if (bridgeRemainingInSrc != null && bridgeRemainingInSrc > 0n)
      effectiveMax = effectiveMax < bridgeRemainingInSrc ? effectiveMax : bridgeRemainingInSrc
    setAmount(formatAmount(effectiveMax.toString(), srcDecimals))
  }, [
    isSourceTerra,
    luncBalance,
    tokenBalance,
    tokenConfig,
    destTokenDetails?.maxTransfer,
    destTokenDetails?.withdrawRateLimit?.remainingAmount,
    amountDecimals,
    toSourceUnits,
  ])

  // EVM deposit success: parse receipt, compute transfer hash, store record, redirect.
  // IMPORTANT: Uses frozenChainsRef (captured at deposit time) to avoid reading stale/swapped
  // sourceChain/destChain from React state — the user may have clicked the swap button
  // while the deposit was in flight.
  useEffect(() => {
    if (evmStatus === 'success' && depositTxHash) {
      setTxHash(depositTxHash)

      // Read frozen chains from ref — these were captured in handleSubmit before the deposit started
      const frozen = frozenChainsRef.current
      const frozenSource = frozen?.sourceChain ?? sourceChain
      const frozenDest = frozen?.destChain ?? destChain
      const frozenDirection = frozen?.direction ?? direction
      const frozenSrcBytes4 = frozen?.sourceChainIdBytes4

      // Try to parse the deposit receipt for nonce and other fields
      const parseAndRecord = async () => {
        let depositNonce: number | undefined
        let depositAmount: string = parseAmount(amount, amountDecimals)
        let tokenAddr: string | undefined = tokenConfig?.address
        let srcAccountHex: string | undefined
        let destAccountHex: string | undefined

        // Parse deposit event from tx receipt using source-chain-specific client
        try {
          const tier0 = DEFAULT_NETWORK as NetworkTier
          const srcChainCfg = BRIDGE_CHAINS[tier0][frozenSource]
          const receiptClient = srcChainCfg?.type === 'evm'
            ? getEvmClient(srcChainCfg)
            : publicClient
          if (receiptClient) {
            const receipt = await receiptClient.getTransactionReceipt({ hash: depositTxHash as `0x${string}` })
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
        let xchainHashId: string | undefined
        if (depositNonce !== undefined) {
          try {
            const tier = DEFAULT_NETWORK as NetworkTier
            const chains = BRIDGE_CHAINS[tier]
            const srcChainConfig = chains[frozenSource]
            const destChainConfig = chains[frozenDest]

            if (srcChainConfig?.bytes4ChainId && destChainConfig?.bytes4ChainId) {
              // Convert bytes4 hex to chain ID number for chainIdToBytes32
              const srcChainIdNum = parseInt(srcChainConfig.bytes4ChainId.slice(2), 16)
              const destChainIdNum = parseInt(destChainConfig.bytes4ChainId.slice(2), 16)

              const srcChainB32 = chainIdToBytes32(srcChainIdNum)
              const destChainB32 = chainIdToBytes32(destChainIdNum)
              const tokenB32 = tokenAddr ? evmAddressToBytes32(tokenAddr as `0x${string}`) : '0x' + '0'.repeat(64)
              const srcAccB32 = srcAccountHex || (evmAddress ? evmAddressToBytes32(evmAddress as `0x${string}`) : '0x' + '0'.repeat(64))
              const destAccB32 = destAccountHex || '0x' + '0'.repeat(64)

              xchainHashId = computeXchainHashId(
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
        const srcChainConfig = chains[frozenSource]
        const destChainConfig = chains[frozenDest]

        // V2 fix: query the correct destination token from TokenRegistry
        // Use source-chain-specific client (not wallet-bound publicClient)
        let destTokenBytes32: string | undefined
        const srcClient = srcChainConfig?.type === 'evm' ? getEvmClient(srcChainConfig) : null
        if (srcClient && tokenAddr && srcChainConfig?.bridgeAddress && destChainConfig?.bytes4ChainId) {
          try {
            const dt = await getDestToken(
              srcClient,
              srcChainConfig.bridgeAddress as Address,
              tokenAddr as Address,
              destChainConfig.bytes4ChainId as `0x${string}`
            )
            if (dt) destTokenBytes32 = dt
          } catch (err) {
            console.warn('[TransferForm] Failed to query destToken from registry:', err)
          }
        }

        recordTransfer({
          type: 'deposit',
          direction: frozenDirection,
          sourceChain: frozenSource,
          destChain: frozenDest,
          amount: depositAmount,
          status: 'confirmed',
          txHash: depositTxHash,
          lifecycle: 'deposited',
          xchainHashId,
          depositNonce,
          srcAccount: srcAccountHex || (evmAddress ? evmAddressToBytes32(evmAddress as `0x${string}`) : undefined),
          destAccount: destAccountHex || recipientAddr,
          token: tokenAddr || selectedTokenId,
          tokenSymbol: tokenConfig?.symbol || getTokenDisplaySymbol(selectedTokenId),
          srcDecimals: amountDecimals,
          destToken: destTokenBytes32 || tokenAddr,
          // For EVM->Terra: store the raw Terra denom so auto-submit can pass it to the Terra contract.
          // For other directions: store the dest token address as-is.
          destTokenId: frozenDirection === 'evm-to-terra'
            ? (selectedTokenId || 'uluna')
            : (destTokenBytes32 ? undefined : tokenAddr),
          destBridgeAddress: destChainConfig?.bridgeAddress,
          sourceChainIdBytes4: frozenSrcBytes4 ?? srcChainConfig?.bytes4ChainId,
        })

        // Clear frozen state after recording
        frozenChainsRef.current = null

        // Redirect to transfer status page (hash only)
        if (xchainHashId) {
          navigate(`/transfer/${xchainHashId}`)
        }
      }

      parseAndRecord()
      resetEvm()
    } else if (evmStatus === 'error' && evmError) {
      setError(evmError)
      frozenChainsRef.current = null
      resetEvm()
    }
  }, [evmStatus, depositTxHash, evmError, resetEvm, sourceChain, destChain, amount, amountDecimals, recordTransfer, direction, navigate, publicClient, evmAddress, tokenConfig, recipientAddr, selectedTokenId])

  // Terra deposit success: parse receipt for nonce, use canonical xchain_hash_id + dest_token_address
  // from the Terra contract's own event attributes (not recomputed by the frontend).
  // Uses frozenChainsRef for the same reason as the EVM handler above.
  useEffect(() => {
    if (terraStatus === 'success' && terraTxHash) {
      setTxHash(terraTxHash)

      // Read frozen chains — for Terra deposits, direction is always terra-to-evm
      // but destChain could have been swapped.
      const frozen = frozenChainsRef.current
      const frozenDest = frozen?.destChain ?? destChain
      const frozenSource = frozen?.sourceChain ?? sourceChain

      const run = async () => {
        console.info(`[TransferForm:terra] Deposit success, txHash=${terraTxHash}`)

        const tier = DEFAULT_NETWORK as NetworkTier
        const chains = BRIDGE_CHAINS[tier]
        const destChainConfig = chains[frozenDest]

        let srcAccountHex: string | undefined
        if (terraAddress) {
          try {
            srcAccountHex = terraAddressToBytes32(terraAddress)
          } catch {
            srcAccountHex = terraAddress
          }
        }

        const destAccountHex = recipientAddr.startsWith('0x')
          ? evmAddressToBytes32(recipientAddr as `0x${string}`)
          : recipientAddr

        // Parse Terra receipt -- extract nonce, amount, and critically the canonical
        // dest_token_address and xchain_hash_id that the Terra contract computed.
        // This eliminates the race condition where registryTokens/destTokenAddr
        // might not be loaded yet, and the wrong EVM token would be used.
        const parsed = await parseTerraLockReceipt(terraTxHash)
        const netAmount = parsed?.amount ?? parseAmount(amount, terraDecimals)
        const depositNonce = parsed?.nonce

        if (!parsed) {
          console.warn(
            `[TransferForm:terra] WARNING: Receipt parsing returned null for tx ${terraTxHash}. ` +
            `Using fallback gross amount=${parseAmount(amount, terraDecimals)}. ` +
            'This may cause a hash mismatch if the Terra contract used a net amount.'
          )
        } else {
          console.info(
            `[TransferForm:terra] parseTerraLockReceipt result: ` +
            `nonce=${depositNonce}, amount=${netAmount}, ` +
            `xchainHashId=${parsed.xchainHashId?.slice(0, 18) ?? 'none'}..., ` +
            `destToken=${parsed.destTokenAddress?.slice(0, 18) ?? 'none'}...`
          )
        }

        // Use the canonical dest_token_address from the Terra contract's event.
        // This is the exact bytes32 EVM token address that Terra used to compute
        // the deposit hash. Using this guarantees hash consistency.
        let destTokenB32: string | undefined = parsed?.destTokenAddress
        if (!destTokenB32) {
          // Fallback for older contracts that don't emit dest_token_address:
          // try destTokenAddr from useTokenDestMapping, then registry
          if (destTokenAddr) {
            destTokenB32 =
              destTokenAddr.length === 66
                ? destTokenAddr
                : evmAddressToBytes32(destTokenAddr as `0x${string}`)
          } else {
            const selectedRegistryToken = registryTokens?.find((t) => t.token === (selectedTokenId || 'uluna'))
            if (selectedRegistryToken?.evm_token_address) {
              const raw = selectedRegistryToken.evm_token_address
              destTokenB32 = raw.startsWith('0x') ? raw : '0x' + raw
            }
          }
        }

        // Use the canonical xchain_hash_id from the Terra contract's event.
        // Only recompute as fallback for older contracts.
        let xchainHashId: string | undefined = parsed?.xchainHashId
        if (!xchainHashId && depositNonce !== undefined && destChainConfig?.bytes4ChainId) {
          try {
            const srcChainCfg = chains[frozenSource]
            const srcChainIdNum = parseInt(frozen?.sourceChainIdBytes4?.slice(2) ?? srcChainCfg?.bytes4ChainId?.slice(2) ?? '1', 16)
            const srcChainB32 = chainIdToBytes32(srcChainIdNum)
            const destChainIdNum = parseInt(destChainConfig.bytes4ChainId.slice(2), 16)
            const destChainB32 = chainIdToBytes32(destChainIdNum)
            const srcAccB32 = (srcAccountHex?.startsWith('0x') && srcAccountHex.length === 66)
              ? srcAccountHex
              : (terraAddress ? terraAddressToBytes32(terraAddress) : '0x' + '0'.repeat(64))
            const destAccB32 = destAccountHex.startsWith('0x') && destAccountHex.length === 66
              ? destAccountHex
              : evmAddressToBytes32(recipientAddr as `0x${string}`)
            const tokenB32 = destTokenB32 && destTokenB32.length === 66
              ? destTokenB32
              : (destTokenAddr ? evmAddressToBytes32(destTokenAddr as `0x${string}`) : '0x' + '0'.repeat(64))
            xchainHashId = computeXchainHashId(
              srcChainB32 as `0x${string}`,
              destChainB32 as `0x${string}`,
              srcAccB32 as `0x${string}`,
              destAccB32 as `0x${string}`,
              tokenB32 as `0x${string}`,
              BigInt(netAmount),
              BigInt(depositNonce)
            )
          } catch (err) {
            console.warn('[TransferForm:terra] Failed to compute Terra transfer hash:', err)
          }
        }

        if (!xchainHashId) {
          console.warn(
            `[TransferForm:terra] WARNING: No xchainHashId computed. ` +
            `nonce=${depositNonce}, bytes4ChainId=${destChainConfig?.bytes4ChainId}, ` +
            `xchainHashId=${parsed?.xchainHashId ?? 'none'}. ` +
            'Transfer will be recorded without a hash and will need nonce resolution on the status page.'
          )
        }

        console.info(
          `[TransferForm:terra] Recording transfer: ` +
          `hash=${xchainHashId?.slice(0, 18) ?? 'none'}..., nonce=${depositNonce}, ` +
          `amount=${netAmount}, destToken=${destTokenB32?.slice(0, 18) ?? 'none'}...`
        )

        recordTransfer({
          type: 'withdrawal',
          direction: 'terra-to-evm',
          sourceChain: frozenSource.includes('terra') || frozenSource.includes('local') ? frozenSource : 'localterra',
          destChain: frozenDest,
          amount: netAmount,
          status: 'confirmed',
          txHash: terraTxHash,
          lifecycle: 'deposited',
          xchainHashId,
          depositNonce,
          srcAccount: srcAccountHex,
          destAccount: destAccountHex,
          token: selectedTokenId || 'uluna',
          tokenSymbol: getTokenDisplaySymbol(selectedTokenId || 'uluna'),
          destToken: destTokenB32,
          srcDecimals: terraDecimals,
          destBridgeAddress: destChainConfig?.bridgeAddress,
          sourceChainIdBytes4: frozen?.sourceChainIdBytes4 ?? chains[frozenSource]?.bytes4ChainId,
        })

        // Clear frozen state after recording
        frozenChainsRef.current = null

        if (xchainHashId) {
          navigate(`/transfer/${xchainHashId}`)
        }
        resetTerra()
      }

      run()
    } else if (terraStatus === 'error' && terraError) {
      setError(terraError)
      frozenChainsRef.current = null
      resetTerra()
    }
  }, [terraStatus, terraTxHash, terraError, resetTerra, navigate, destChain, sourceChain, amount, terraDecimals, recordTransfer, terraAddress, recipientAddr, selectedTokenId, registryTokens, destTokenAddr])

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

    // Freeze chain state at deposit time so that subsequent UI changes
    // (e.g. swap button) don't corrupt the TransferRecord.
    const tier0 = DEFAULT_NETWORK as NetworkTier
    const srcConfig0 = BRIDGE_CHAINS[tier0][sourceChain]
    frozenChainsRef.current = {
      sourceChain,
      destChain,
      direction,
      sourceChainIdBytes4: srcConfig0?.bytes4ChainId,
    }

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
      if (!v2Bytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        return
      }
      const destChainId = parseInt(v2Bytes4, 16)
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
      // Use V2 bytes4 chain ID from config only.
      // Do NOT derive from native chain ID (e.g., 31337/31338), since cross-chain
      // routing/hashing must use V2 IDs.
      const destChainBytes4 = destConfig?.bytes4ChainId as Hex | undefined
      if (!destChainBytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        return
      }
      const destAccount = encodeEvmAddress(recipientAddr)
      await evmDeposit(amount, destChainBytes4, destAccount, tokenConfig.decimals)
    }
  }

  const balanceDisplay = isSourceTerra
    ? formatAmount(luncBalance, terraDecimals)
    : tokenBalance !== undefined && tokenConfig
    ? formatAmount(tokenBalance.toString(), tokenConfig.decimals)
    : undefined

  // Use onchain/tokenlist display hooks for symbol (not raw address)
  const selectedSymbol =
    isSourceTerra
      ? terraDisplay.displayLabel || transferTokens.find((t) => t.id === selectedTokenId)?.symbol || 'LUNC'
      : (evmSourceDisplay.displayLabel || tokenConfig?.symbol || transferTokens.find((t) => t.id === selectedTokenId)?.symbol || 'LUNC')
  const walletLabel = isSourceTerra ? 'Terra' : 'EVM'

  // "You will receive" shows DESTINATION token - use display hooks for symbol, link to dest chain
  const feeBreakdownProps = useMemo(() => {
    const destChainInfo = allChains.find((c) => c.id === destChain)
    const destType = destChainInfo?.type ?? 'evm'
    const tokenAddr =
      destType === 'cosmos' && selectedTokenId?.startsWith('terra1')
        ? selectedTokenId
        : destTokenAddr

    const display =
      destType === 'cosmos'
        ? terraDisplay.displayLabel || (tokenlist ? getTokenFromList(tokenlist, selectedTokenId)?.symbol : null) || selectedSymbol
        : evmDestDisplay.displayLabel || (tokenlist ? getTokenFromList(tokenlist, selectedTokenId)?.symbol : null) || shortenAddress(tokenAddr)

    const url = destChainInfo?.explorerUrl && tokenAddr
      ? getTokenExplorerUrl(destChainInfo.explorerUrl, tokenAddr, destType)
      : ''

    return {
      destDisplaySymbol: display,
      tokenId: destType === 'cosmos' ? selectedTokenId : tokenAddr,
      tokenExplorerUrl: url,
    }
  }, [
    tokenlist,
    selectedTokenId,
    selectedSymbol,
    destChain,
    allChains,
    destTokenAddr,
    terraDisplay.displayLabel,
    evmDestDisplay.displayLabel,
  ])

  const buttonText = isChainsLoading
    ? 'Discovering chains...'
    : !isWalletConnected
    ? `Connect ${walletLabel} Wallet`
    : isSourceEvm && (isRegistryLoading || isSourceMappingsLoading)
    ? 'Loading token info...'
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
        disabled={isSubmitting}
      />
      <AmountInput
        value={amount}
        onChange={setAmount}
        onMax={
          isSourceTerra && luncBalance
            ? handleMax
            : tokenBalance !== undefined && tokenConfig
            ? handleMax
            : undefined
        }
        tokens={transferTokens}
        selectedTokenId={selectedTokenId}
        onTokenChange={setSelectedTokenId}
        symbol={selectedSymbol}
        sourceChainRpcUrl={!isSourceTerra && sourceChainConfig?.type === 'evm' ? sourceChainConfig.rpcUrl : undefined}
        maxLabel={displayMaxLabel}
        minLabel={displayMinLabel}
      />
      <SwapDirectionButton onClick={handleSwap} disabled={isSwapDisabled || isSubmitting} />
      <DestChainSelector chains={destChains} value={destChain} onChange={setDestChain} disabled={isSubmitting} />
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
      <FeeBreakdown
        receiveAmount={receiveAmount}
        symbol={feeBreakdownProps.destDisplaySymbol}
        tokenId={feeBreakdownProps.tokenId}
        destChain={destChain}
        tokenExplorerUrl={feeBreakdownProps.tokenExplorerUrl}
      />

      <button
        type="submit"
        data-testid="submit-transfer"
        disabled={isChainsLoading || !isWalletConnected || !amount || !isValidAmount(amount) || isSubmitting || (isSourceEvm && (isRegistryLoading || isSourceMappingsLoading))}
        onClick={() => sounds.playButtonPress()}
        className="btn-primary btn-cta w-full justify-center py-3 disabled:bg-gray-700 disabled:text-gray-400 disabled:shadow-none disabled:translate-x-0 disabled:translate-y-0 disabled:cursor-not-allowed"
      >
        {buttonText}
      </button>
    </form>
  )
}
