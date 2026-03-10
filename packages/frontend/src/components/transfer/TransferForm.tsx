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
import { useCw20Balance } from '../../hooks/useContract'
import { useTerraTokenDisplayInfo, useEvmTokenDisplayInfo } from '../../hooks/useTokenDisplayInfo'
import {
  useBridgeDeposit,
  computeTerraChainBytes4,
  encodeTerraAddress,
  encodeEvmAddress,
} from '../../hooks/useBridgeDeposit'
import { useTransferRouteValidation } from '../../hooks/useTransferRouteValidation'
import { useTerraDeposit } from '../../hooks/useTerraDeposit'
import { useTransferStore } from '../../stores/transfer'
import { useUIStore } from '../../stores/ui'
import { getChainById } from '../../lib/chains'
import { getChainsForTransfer, BRIDGE_CHAINS, type NetworkTier } from '../../utils/bridgeChains'
import { useDiscoveredChains } from '../../hooks/useDiscoveredChains'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'
import { getTokenFromList, getTerraAddressFromList, type TokenlistData } from '../../services/tokenlist'
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

function deriveDirection(source: ChainInfo | undefined, dest: ChainInfo | undefined): TransferDirection {
  if (!source || !dest) return 'terra-to-evm'
  if (source.type === 'cosmos' && dest.type === 'evm') return 'terra-to-evm'
  if (source.type === 'evm' && dest.type === 'cosmos') return 'evm-to-terra'
  return 'evm-to-evm'
}

function getValidDestChains(allChains: ChainInfo[], sourceChainId: string): ChainInfo[] {
  const source = allChains.find((c) => c.id === sourceChainId)
  return allChains.filter((c) => {
    if (c.id === sourceChainId) return false
    if (source?.type === 'cosmos' && c.type === 'cosmos') return false
    return true
  })
}

const LOCK_UNLOCK_ADDRESS = (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address

function buildTransferTokens(
  registryTokens: { token: string; is_native: boolean; evm_token_address?: string; terra_decimals: number; evm_decimals?: number; enabled: boolean }[] | undefined,
  isSourceTerra: boolean,
  fallbackConfig: { address: Address; symbol: string; decimals: number } | undefined,
  tokenlist: TokenlistData | null,
  sourceChainMappings?: Record<string, string>,
  destChainMappings?: Record<string, string>
): TokenOption[] {
  const symbolFromList = (token: string) =>
    tokenlist ? getTokenFromList(tokenlist, token)?.symbol : null
  if (isSourceTerra) {
    if (!tokenlist) return []
    let enabledTokens = (registryTokens ?? []).filter((t) => t.enabled)
    if (destChainMappings && Object.keys(destChainMappings).length > 0) {
      enabledTokens = enabledTokens.filter((t) => t.token in destChainMappings)
    }
    const fromRegistry = enabledTokens.map((t) => ({
      id: t.token,
      symbol: symbolFromList(t.token) ?? getTokenDisplaySymbol(t.token),
      tokenId: t.token,
    }))
    if (fromRegistry.length > 0) return fromRegistry
    return []
  }
  if (!tokenlist) return []
  if (sourceChainMappings && Object.keys(sourceChainMappings).length > 0) {
    return Object.entries(sourceChainMappings).map(([terraToken, evmAddr]) => {
      const reg = registryTokens?.find((t) => t.token === terraToken)
      return {
        id: terraToken,
        symbol: symbolFromList(terraToken) ?? getTokenDisplaySymbol(reg?.token ?? terraToken),
        tokenId: terraToken,
        evmTokenAddress: evmAddr,
      }
    })
  }
  const baseRegistry = (registryTokens ?? []).filter((t) => t.enabled && t.evm_token_address)
  if (baseRegistry.length > 0) {
    return baseRegistry.map((t) => ({
      id: t.token,
      symbol: symbolFromList(t.token) ?? getTokenDisplaySymbol(t.token),
      tokenId: t.token,
      evmTokenAddress: bytes32ToAddress(t.evm_token_address!),
    }))
  }
  if (fallbackConfig && !sourceChainMappings) {
    return [{ id: fallbackConfig.address, symbol: fallbackConfig.symbol, tokenId: fallbackConfig.address }]
  }
  return []
}

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

function getEvmTokenConfig(
  selectedTokenId: string,
  registryTokens: { token: string; evm_token_address?: string; evm_decimals?: number }[] | undefined,
  fallbackConfig: { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined,
  sourceChainMappings?: Record<string, string>,
  sourceChainDecimals?: Record<string, number>
): { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined {
  const fromRegistry = registryTokens?.find((t) => t.token === selectedTokenId)
  const mappedAddr = sourceChainMappings?.[selectedTokenId]
  const evmAddr = mappedAddr ?? fromRegistry?.evm_token_address
  if (evmAddr) {
    return {
      address: (mappedAddr ?? bytes32ToAddress(evmAddr)) as Address,
      lockUnlockAddress: LOCK_UNLOCK_ADDRESS,
      symbol: getTokenDisplaySymbol(fromRegistry?.token ?? selectedTokenId),
      decimals: sourceChainDecimals?.[selectedTokenId] ?? fromRegistry?.evm_decimals ?? 18,
    }
  }
  if (fallbackConfig && (selectedTokenId === fallbackConfig.address || selectedTokenId === 'uluna')) {
    return fallbackConfig
  }
  return undefined
}

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
  const { recordTransfer, setActiveTransfer, updateActiveTransfer } = useTransferStore()
  const navigate = useNavigate()
  const publicClient = usePublicClient()

  const { chains: allChains, isLoading: isChainsLoading } = useDiscoveredChains()
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

  const frozenChainsRef = useRef<{
    sourceChain: string
    destChain: string
    direction: TransferDirection
    sourceChainIdBytes4: string | undefined
  } | null>(null)

  const destChains = useMemo(() => getValidDestChains(allChains, sourceChain), [allChains, sourceChain])

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
  const destChainConfig = useMemo(
    () => BRIDGE_CHAINS[DEFAULT_NETWORK as NetworkTier]?.[destChain],
    [destChain]
  )
  const { data: bridgeConfigData } = useBridgeConfig()
  const isSourceEvm = sourceChainConfig?.type === 'evm'
  const sourceChainBytes4 = sourceChainConfig?.bytes4ChainId
  const destChainBytes4 = destChainConfig?.bytes4ChainId
  const isDestEvmChain = destChainConfig?.type === 'evm'
  const { mappings: sourceChainMappings, decimalsMap: sourceChainDecimals, isLoading: isSourceMappingsLoading } = useSourceChainTokenMappings(
    registryTokens,
    sourceChainBytes4,
    !!isSourceEvm && !!sourceChainBytes4
  )
  const { mappings: destChainMappings, decimalsMap: destChainDecimals, isLoading: isDestMappingsLoading } = useSourceChainTokenMappings(
    registryTokens,
    destChainBytes4,
    !!isDestEvmChain && !!destChainBytes4
  )

  const fallbackTokenConfig = TOKEN_CONFIGS[DEFAULT_NETWORK]
  const readySourceMappings = isSourceEvm && !isSourceMappingsLoading ? sourceChainMappings : undefined
  const readyDestMappings = isSourceTerra && isDestEvmChain && !isDestMappingsLoading ? destChainMappings : undefined
  const transferTokens = useMemo(
    () =>
      buildTransferTokens(
        registryTokens,
        isSourceTerra,
        fallbackTokenConfig,
        tokenlist ?? null,
        readySourceMappings,
        readyDestMappings
      ),
    [registryTokens, isSourceTerra, fallbackTokenConfig, tokenlist, readySourceMappings, readyDestMappings]
  )

  const readySourceDecimals = isSourceEvm && !isSourceMappingsLoading ? sourceChainDecimals : undefined
  const evmTokenConfig = useMemo(
    () =>
      getEvmTokenConfig(
        selectedTokenId,
        registryTokens,
        fallbackTokenConfig,
        readySourceMappings,
        readySourceDecimals
      ),
    [selectedTokenId, registryTokens, fallbackTokenConfig, readySourceMappings, readySourceDecimals]
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
  const terraCw20Address = useMemo(() => {
    if (!selectedTokenId || selectedTokenId === 'uluna') return null
    if (selectedTokenId.startsWith('terra1')) return selectedTokenId
    const cw20Match = selectedTokenId.match(/cw20:(terra1[a-z0-9]+)/i)
    if (cw20Match?.[1]) return cw20Match[1]
    const symbol = transferTokens.find((t) => t.id === selectedTokenId)?.symbol
    return getTerraAddressFromList(tokenlist ?? null, selectedTokenId, symbol) ?? null
  }, [selectedTokenId, transferTokens, tokenlist])

  const { data: terraCw20Balance } = useCw20Balance(
    terraAddress ?? undefined,
    terraCw20Address ?? undefined,
    isSourceTerra && !!terraCw20Address
  )

  const terraSourceBalance = isSourceTerra
    ? (selectedTokenId === 'uluna' ? luncBalance : (terraCw20Balance ?? '0'))
    : '0'

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

  const sourceBridgeConfig = useMemo(() => {
    const tier = DEFAULT_NETWORK as NetworkTier
    const config = BRIDGE_CHAINS[tier][sourceChain]
    if (!config || config.type !== 'evm') return undefined
    return {
      bridgeAddress: config.bridgeAddress as Address,
      nativeChainId: typeof config.chainId === 'number' ? config.chainId : undefined,
      rpcUrl: config.rpcUrl,
      sourceChainConfig: config,
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
          sourceChainConfig: sourceBridgeConfig?.sourceChainConfig,
        }
      : undefined
  )
  const { lock: terraLock, status: terraStatus, txHash: terraTxHash, error: terraError, reset: resetTerra } = useTerraDeposit()

  const isWalletConnected = isSourceTerra ? isTerraConnected : isEvmConnected

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
    evmStatus === 'switching-chain' ||
    evmStatus === 'checking-allowance' ||
    evmStatus === 'approving' ||
    evmStatus === 'waiting-approval' ||
    evmStatus === 'depositing' ||
    evmStatus === 'waiting-deposit' ||
    terraStatus === 'locking'

  const amountDecimals = isSourceTerra ? terraDecimals : (tokenConfig?.decimals ?? DECIMALS.LUNC)
  const destDecimals = isDestEvmChain
    ? (destChainDecimals?.[selectedTokenId]
        ?? registryTokens?.find((t) => t.token === selectedTokenId)?.evm_decimals
        ?? 18)
    : terraDecimals

  const toSourceUnits = useCallback(
    (destBaseUnits: bigint) => {
      if (amountDecimals >= destDecimals) {
        const exp = amountDecimals - destDecimals
        return destBaseUnits * BigInt(10 ** exp)
      }
      const exp = destDecimals - amountDecimals
      const result = destBaseUnits / BigInt(10 ** exp)
      if (result === 0n && destBaseUnits > 0n) return 1n
      return result
    },
    [amountDecimals, destDecimals]
  )

  const { displayMaxLabel, displayBridgeMax, displayMinLabel, effectiveMinInSrc, effectiveMaxInSrc } = useMemo(() => {
    const srcDecimals = amountDecimals
    let balanceStr: string | undefined
    if (isSourceTerra) {
      balanceStr = terraSourceBalance
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

    const bridgeLimit = bridgeRemainingInSrc ?? maxTransferInSrc
    return {
      displayMaxLabel:
        effectiveMax > 0n ? formatCompact(effectiveMax.toString(), srcDecimals) : undefined,
      displayBridgeMax:
        bridgeLimit != null && bridgeLimit > 0n ? formatCompact(bridgeLimit.toString(), srcDecimals) : undefined,
      displayMinLabel:
        effectiveMin != null && effectiveMin > 0n
          ? formatCompact(effectiveMin.toString(), srcDecimals)
          : undefined,
      effectiveMinInSrc: effectiveMin,
      effectiveMaxInSrc: effectiveMax,
    }
  }, [
    isSourceTerra,
    terraSourceBalance,
    tokenBalance,
    tokenConfig,
    destTokenDetails?.maxTransfer,
    destTokenDetails?.minTransfer,
    destTokenDetails?.withdrawRateLimit?.remainingAmount,
    amountDecimals,
    toSourceUnits,
  ])

  const isBelowMin = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return false
    const parsed = BigInt(parseAmount(amount, amountDecimals))
    if (effectiveMinInSrc != null && effectiveMinInSrc > 0n) {
      return parsed < effectiveMinInSrc
    }
    return false
  }, [amount, amountDecimals, effectiveMinInSrc])

  const isAboveMax = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return false
    if (effectiveMaxInSrc <= 0n) return false
    const parsed = BigInt(parseAmount(amount, amountDecimals))
    return parsed > effectiveMaxInSrc
  }, [amount, amountDecimals, effectiveMaxInSrc])

  const handleMax = useCallback(() => {
    const srcDecimals = amountDecimals
    let balanceStr: string
    if (isSourceTerra) {
      balanceStr = terraSourceBalance
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
    terraSourceBalance,
    tokenBalance,
    tokenConfig,
    destTokenDetails?.maxTransfer,
    destTokenDetails?.withdrawRateLimit?.remainingAmount,
    amountDecimals,
    toSourceUnits,
  ])

  useEffect(() => {
    if (!isSourceTerra && amount && tokenConfig) {
      setActiveTransfer({
        id: `evm-deposit-${Date.now()}`,
        direction,
        sourceChain,
        destChain,
        amount: parseAmount(amount, amountDecimals),
        status: 'pending',
        txHash: depositTxHash ?? null,
        recipient: recipientAddr,
        startedAt: Date.now(),
        srcDecimals: amountDecimals,
        tokenSymbol: tokenConfig.symbol || getTokenDisplaySymbol(selectedTokenId),
      })
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [evmStatus === 'approving' || evmStatus === 'waiting-approval' || evmStatus === 'depositing' || evmStatus === 'waiting-deposit'])

  useEffect(() => {
    if ((evmStatus === 'waiting-approval' || evmStatus === 'waiting-deposit') && depositTxHash) {
      updateActiveTransfer({ txHash: depositTxHash })
    }
  }, [evmStatus, depositTxHash, updateActiveTransfer])

  useEffect(() => {
    if (evmStatus === 'success' && depositTxHash) {
      setTxHash(depositTxHash)
      updateActiveTransfer({ txHash: depositTxHash, status: 'confirmed' })

      const frozen = frozenChainsRef.current
      const frozenSource = frozen?.sourceChain ?? sourceChain
      const frozenDest = frozen?.destChain ?? destChain
      const frozenDirection = frozen?.direction ?? direction
      const frozenSrcBytes4 = frozen?.sourceChainIdBytes4

      const parseAndRecord = async () => {
        let depositNonce: number | undefined
        let depositAmount: string = parseAmount(amount, amountDecimals)
        let tokenAddr: string | undefined = tokenConfig?.address
        let srcAccountHex: string | undefined
        let destAccountHex: string | undefined

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

        const tier = DEFAULT_NETWORK as NetworkTier
        const chains = BRIDGE_CHAINS[tier]
        const srcChainConfig = chains[frozenSource]
        const destChainConfig = chains[frozenDest]

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

        let xchainHashId: string | undefined
        if (depositNonce !== undefined) {
          try {
            if (srcChainConfig?.bytes4ChainId && destChainConfig?.bytes4ChainId) {
              const srcChainIdNum = parseInt(srcChainConfig.bytes4ChainId.slice(2), 16)
              const destChainIdNum = parseInt(destChainConfig.bytes4ChainId.slice(2), 16)

              const srcChainB32 = chainIdToBytes32(srcChainIdNum)
              const destChainB32 = chainIdToBytes32(destChainIdNum)
              const tokenB32 = destTokenBytes32
                || (tokenAddr ? evmAddressToBytes32(tokenAddr as `0x${string}`) : '0x' + '0'.repeat(64))
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
          destTokenId: frozenDirection === 'evm-to-terra'
            ? (selectedTokenId || 'uluna')
            : (destTokenBytes32 ? undefined : tokenAddr),
          destBridgeAddress: destChainConfig?.bridgeAddress,
          sourceChainIdBytes4: frozenSrcBytes4 ?? srcChainConfig?.bytes4ChainId,
        })

        frozenChainsRef.current = null
        setActiveTransfer(null)

        if (xchainHashId) {
          navigate(`/transfer/${xchainHashId}`)
        }
      }

      parseAndRecord()
      resetEvm()
    } else if (evmStatus === 'error' && evmError) {
      setError(evmError)
      updateActiveTransfer({ status: 'failed' })
      setActiveTransfer(null)
      frozenChainsRef.current = null
      resetEvm()
    }
  }, [evmStatus, depositTxHash, evmError, resetEvm, sourceChain, destChain, amount, amountDecimals, recordTransfer, direction, navigate, publicClient, evmAddress, tokenConfig, recipientAddr, selectedTokenId, updateActiveTransfer, setActiveTransfer, isSourceTerra])

  useEffect(() => {
    if (terraStatus === 'success' && terraTxHash) {
      setTxHash(terraTxHash)

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

        let destTokenB32: string | undefined = parsed?.destTokenAddress
        if (!destTokenB32) {
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

  const isSwapDisabled = useMemo(() => {
    const destInfo = getChainById(destChain)
    const sourceInfo = getChainById(sourceChain)
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
    if (isBelowMin) {
      setError(`Amount is below the minimum transfer amount${displayMinLabel ? ` (${displayMinLabel})` : ''}`)
      return
    }
    if (isAboveMax) {
      setError(`Amount exceeds the maximum${displayMaxLabel ? ` (${displayMaxLabel})` : ''}`)
      return
    }
    if (submitGuardError) {
      setError(submitGuardError)
      return
    }

    const tier0 = DEFAULT_NETWORK as NetworkTier
    const srcConfig0 = BRIDGE_CHAINS[tier0][sourceChain]
    frozenChainsRef.current = {
      sourceChain,
      destChain,
      direction,
      sourceChainIdBytes4: srcConfig0?.bytes4ChainId,
    }

    const tier = DEFAULT_NETWORK as NetworkTier
    const destConfig = BRIDGE_CHAINS[tier][destChain]

    if (direction === 'terra-to-evm') {
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      const v2Bytes4 = destConfig?.bytes4ChainId
      if (!v2Bytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        return
      }
      const destChainId = parseInt(v2Bytes4, 16)
      const amountMicro = parseAmount(amount, terraDecimals)
      const selectedReg = registryTokens?.find((t) => t.token === selectedTokenId)
      const isNative = selectedReg?.is_native ?? (selectedTokenId === 'uluna')
      await terraLock({
        amountMicro,
        destChainId,
        recipientEvm: recipientAddr,
        tokenId: selectedTokenId || 'uluna',
        isNative,
        srcDecimals: terraDecimals,
        tokenSymbol:
          terraDisplay.displayLabel ||
          transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
          getTokenDisplaySymbol(selectedTokenId || 'uluna'),
      })
    } else if (direction === 'evm-to-terra') {
      if (!recipientAddr || !recipientAddr.startsWith('terra1')) {
        setError('Please provide a Terra recipient address or connect your Terra wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      setActiveTransfer({
        id: `evm-deposit-${Date.now()}`,
        direction,
        sourceChain,
        destChain,
        amount: parseAmount(amount, tokenConfig.decimals),
        status: 'pending',
        txHash: null,
        recipient: recipientAddr,
        startedAt: Date.now(),
        srcDecimals: tokenConfig.decimals,
        tokenSymbol: tokenConfig.symbol || getTokenDisplaySymbol(selectedTokenId),
      })
      const destChainBytes4 = computeTerraChainBytes4()
      const destAccount = encodeTerraAddress(recipientAddr)
      await evmDeposit(amount, destChainBytes4, destAccount, tokenConfig.decimals)
    } else {
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      const destChainBytes4 = destConfig?.bytes4ChainId as Hex | undefined
      if (!destChainBytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        return
      }
      setActiveTransfer({
        id: `evm-deposit-${Date.now()}`,
        direction,
        sourceChain,
        destChain,
        amount: parseAmount(amount, tokenConfig.decimals),
        status: 'pending',
        txHash: null,
        recipient: recipientAddr,
        startedAt: Date.now(),
        srcDecimals: tokenConfig.decimals,
        tokenSymbol: tokenConfig.symbol || getTokenDisplaySymbol(selectedTokenId),
      })
      const destAccount = encodeEvmAddress(recipientAddr)
      await evmDeposit(amount, destChainBytes4, destAccount, tokenConfig.decimals)
    }
  }

  const balanceDisplay = isSourceTerra
    ? formatAmount(terraSourceBalance, terraDecimals)
    : tokenBalance !== undefined && tokenConfig
    ? formatAmount(tokenBalance.toString(), tokenConfig.decimals)
    : undefined

  const selectedSymbol =
    isSourceTerra
      ? terraDisplay.displayLabel || transferTokens.find((t) => t.id === selectedTokenId)?.symbol || 'LUNC'
      : (evmSourceDisplay.displayLabel || tokenConfig?.symbol || transferTokens.find((t) => t.id === selectedTokenId)?.symbol || '—')
  const walletLabel = isSourceTerra ? 'Terra' : 'EVM'

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

  const isTokenInfoLoading =
    (isSourceEvm && (isRegistryLoading || isSourceMappingsLoading)) ||
    (isSourceTerra && (isRegistryLoading || isDestMappingsLoading))
  const {
    isValid: isRouteValid,
    error: routeValidationError,
    isLoading: isRouteValidationLoading,
  } = useTransferRouteValidation({
    enabled:
      !!selectedTokenId &&
      !!sourceChainConfig &&
      !!destChainConfig &&
      !isChainsLoading &&
      !isTokenInfoLoading,
    tokenLabel: selectedSymbol,
    sourceChainConfig,
    destChainConfig,
    sourceTokenAddress: tokenConfig?.address,
    sourceMappingAddress: readySourceMappings?.[selectedTokenId],
    destTokenAddress: destChainConfig?.type === 'evm' ? (destTokenAddr || undefined) : undefined,
    destMappingAddress: destChainConfig?.type === 'evm' ? (tokenDestMappingAddr || undefined) : undefined,
    destTokenId: destChainConfig?.type === 'cosmos' ? ((terraCw20Address ?? selectedTokenId) || undefined) : undefined,
  })
  const submitGuardError =
    !isTokenInfoLoading && !isRouteValidationLoading && !isRouteValid
      ? routeValidationError
      : null

  const buttonText = isChainsLoading
    ? 'Discovering chains...'
    : !isWalletConnected
    ? `Connect ${walletLabel} Wallet`
    : isTokenInfoLoading
    ? 'Loading token info...'
    : isRouteValidationLoading
    ? 'Validating route...'
    : submitGuardError
    ? 'Route misconfigured'
    : evmStatus === 'switching-chain'
    ? `Switching to ${sourceChainConfig?.name ?? 'source chain'}...`
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
        <div className="bg-red-900/30 border-2 border-red-700 p-3 overflow-hidden">
          <p className="text-red-300 text-sm break-all">{error}</p>
          <button type="button" onClick={() => setError(null)} className="text-red-200 text-xs mt-2 underline underline-offset-2">
            Dismiss
          </button>
        </div>
      )}
      {!error && submitGuardError && (
        <div className="bg-amber-900/30 border-2 border-amber-700 p-3">
          <p className="text-amber-200 text-sm">{submitGuardError}</p>
          <p className="text-amber-300/70 text-xs mt-1">
            This route is blocked until the token mapping resolves to a real destination token.
          </p>
        </div>
      )}

      <SourceChainSelector
        chains={allChains}
        value={sourceChain}
        onChange={handleSourceChange}
        balance={balanceDisplay}
        balanceLabel={selectedSymbol}
        bridgeMax={displayBridgeMax}
        periodEndsAt={destTokenDetails?.withdrawRateLimit?.periodEndsAt}
        fetchedAtWallMs={destTokenDetails?.withdrawRateLimit?.fetchedAtWallMs}
        disabled={isSubmitting}
      />
      <AmountInput
        value={amount}
        onChange={setAmount}
        onMax={
          isSourceTerra && terraSourceBalance
            ? handleMax
            : tokenBalance !== undefined && tokenConfig
            ? handleMax
            : undefined
        }
        tokens={transferTokens}
        selectedTokenId={selectedTokenId}
        onTokenChange={setSelectedTokenId}
        symbol={selectedSymbol}
        sourceChainConfigOrRpcUrl={!isSourceTerra && sourceChainConfig?.type === 'evm' ? sourceChainConfig : undefined}
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
        disabled={
          isChainsLoading ||
          !isWalletConnected ||
          !amount ||
          !isValidAmount(amount) ||
          isBelowMin ||
          isAboveMax ||
          isSubmitting ||
          isTokenInfoLoading ||
          isRouteValidationLoading ||
          !isRouteValid
        }
        onClick={() => sounds.playButtonPress()}
        className="btn-primary btn-cta w-full justify-center py-3 disabled:bg-gray-700 disabled:text-gray-400 disabled:shadow-none disabled:translate-x-0 disabled:translate-y-0 disabled:cursor-not-allowed"
      >
        {buttonText}
      </button>
    </form>
  )
}
