import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAccount, usePublicClient } from 'wagmi'
import { useNavigate } from 'react-router-dom'
import { Address, getAddress, type Hex, hexToBytes } from 'viem'
import { useWallet } from '../../hooks/useWallet'
import { useTokenRegistry } from '../../hooks/useTokenRegistry'
import { useTokenList } from '../../hooks/useTokenList'
import { useTokenDestMapping, useTokenDestMappingRaw } from '../../hooks/useTokenDestMapping'
import { useSourceChainTokenMappings } from '../../hooks/useSourceChainTokenMappings'
import { useBridgeConfig, useTokenDetails } from '../../hooks/useBridgeConfig'
import { useCw20Balance } from '../../hooks/useContract'
import { useTerraTokenDisplayInfo, useEvmTokenDisplayInfo } from '../../hooks/useTokenDisplayInfo'
import { useBridgeDeposit, encodeTerraAddress, encodeEvmAddress } from '../../hooks/useBridgeDeposit'
import { useTransferRouteValidation } from '../../hooks/useTransferRouteValidation'
import { useTerraDeposit } from '../../hooks/useTerraDeposit'
import { useSolanaWallet } from '../../hooks/useSolanaWallet'
import { useSolanaDeposit } from '../../hooks/useSolanaDeposit'
import {
  WSOL_MINT,
  bytes4HexToUint8Array,
  fetchDepositNonce,
  fetchSplMintDecimals,
  fetchTokenMappingLocalMint,
  formatSolanaWalletError,
} from '../../services/solana/transaction'
import {
  pickSolanaConnection,
  solanaRpcUrlsForBridgeChain,
  withSolanaReadFallback,
} from '../../services/solana/solanaRpcUrls'
import { getSolanaProgramIdString } from '../../services/solana/solanaBridgeAccounts'
import { bytes32ToSolanaAddress, solanaAddressToBytes32 } from '../../services/solana/address'
import { hexToUint8Array } from '../../services/terra/withdrawSubmit'
import { resolveTerraDestTokenIdForRecord } from '../../services/terra/withdrawTokenResolve'
import { PublicKey } from '@solana/web3.js'
import { useTransferStore } from '../../stores/transfer'
import { useUIStore } from '../../stores/ui'
import { getChainById, getExplorerTxUrl } from '../../lib/chains'
import { getChainsForTransfer, BRIDGE_CHAINS, type NetworkTier } from '../../utils/bridgeChains'
import { useDiscoveredChains } from '../../hooks/useDiscoveredChains'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'
import { getTokenFromList, getTerraAddressFromList } from '../../services/tokenlist'
import { buildTransferTokens } from '../../services/transfer/buildTransferTokens'
import { shortenAddress } from '../../utils/shortenAddress'
import { getTokenExplorerUrl } from '../../utils/format'
import { parseDepositFromLogs } from '../../services/evm/depositReceipt'
import { parseTerraLockReceipt } from '../../services/terra/depositReceipt'
import { getDestToken } from '../../services/evm/tokenRegistry'
import { getEvmClient } from '../../services/evmClient'
import {
  computeXchainHashId,
  chainIdToBytes32,
  computeXchainHashIdFromBytes,
  evmAddressToBytes32,
  terraAddressToBytes32,
} from '../../services/hashVerification'
import type { ChainInfo } from '../../lib/chains'
import type { TransferDirection } from '../../types/transfer'
import { DEFAULT_NETWORK, BRIDGE_CONFIG, DECIMALS } from '../../utils/constants'
import {
  parseAmount,
  parseAmountAsBigInt,
  formatAmount,
  formatAmountForNumberInput,
  formatCompact,
} from '../../utils/format'
import { pow10BigInt } from '../../utils/pow10'
import { bigintFromBaseUnitsString } from '../../utils/scientificDecimal'
import { isValidAmount } from '../../utils/validation'
import { minGrossForMinNet } from '../../utils/bridgeMinAmount'
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
  if (source.type === 'solana' && dest.type === 'evm') return 'solana-to-evm'
  if (source.type === 'evm' && dest.type === 'solana') return 'evm-to-solana'
  if (source.type === 'solana' && dest.type === 'cosmos') return 'solana-to-terra'
  if (source.type === 'cosmos' && dest.type === 'solana') return 'terra-to-solana'
  if (source.type === 'cosmos' && dest.type === 'evm') return 'terra-to-evm'
  if (source.type === 'evm' && dest.type === 'cosmos') return 'evm-to-terra'
  return 'evm-to-evm'
}

/** Get valid destination chains for a given source chain */
function getValidDestChains(allChains: ChainInfo[], sourceChainId: string): ChainInfo[] {
  const source = allChains.find((c) => c.id === sourceChainId)
  return allChains.filter((c) => {
    if (c.id === sourceChainId) return false
    // Cosmos → Cosmos not supported
    if (source?.type === 'cosmos' && c.type === 'cosmos') return false
    // Solana → Solana not supported (same chain type bridge)
    if (source?.type === 'solana' && c.type === 'solana') return false
    return true
  })
}

const ZERO_LOCK = '0x0000000000000000000000000000000000000000' as Address

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
  registryTokens: { token: string; evm_token_address?: string; evm_decimals?: number }[] | undefined,
  fallbackConfig: { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined,
  /** LockUnlock on the selected source EVM chain (Anvil vs Anvil1 differ). */
  lockUnlockAddress: Address,
  /** When source is EVM: use per-chain address from token_dest_mapping */
  sourceChainMappings?: Record<string, string>,
  /** Per-chain decimals from token_dest_mapping */
  sourceChainDecimals?: Record<string, number>,
): { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined {
  const fromRegistry = registryTokens?.find((t) => t.token === selectedTokenId)
  const mappedAddr = sourceChainMappings?.[selectedTokenId]
  const evmAddr = mappedAddr ?? fromRegistry?.evm_token_address
  if (evmAddr) {
    return {
      address: (mappedAddr ?? bytes32ToAddress(evmAddr)) as Address,
      lockUnlockAddress,
      symbol: getTokenDisplaySymbol(fromRegistry?.token ?? selectedTokenId),
      decimals: sourceChainDecimals?.[selectedTokenId] ?? fromRegistry?.evm_decimals ?? 18,
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
  const {
    connected: isSolanaConnected,
    address: solanaAddress,
    walletType: solanaWalletType,
    setShowWalletModal: setShowSolanaModal,
  } = useSolanaWallet()
  const {
    step: solanaStep,
    txSignature: solanaTxSig,
    confirmedDepositNonce,
    deposit: solanaDeposit,
    reset: resetSolana,
  } = useSolanaDeposit()
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

  const isSourceTerra = direction === 'terra-to-evm' || direction === 'terra-to-solana'
  const isSourceSolana = direction === 'solana-to-evm' || direction === 'solana-to-terra'
  const isDestEvm = direction === 'terra-to-evm' || direction === 'evm-to-evm' || direction === 'solana-to-evm'
  const isDestSolana = direction === 'evm-to-solana' || direction === 'terra-to-solana'

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
  // Query dest chain mappings for token filtering (Terra→EVM / Solana→* ) and dest decimals (all→EVM).
  const isDestSolanaChain = destChainConfig?.type === 'solana'
  const isDestCosmosChain = destChainConfig?.type === 'cosmos'
  const destMappingsQueryEnabled =
    !!destChainBytes4 &&
    (isDestEvmChain || isDestSolanaChain || (isSourceSolana && isDestCosmosChain))
  const { mappings: destChainMappings, decimalsMap: destChainDecimals, isLoading: isDestMappingsLoading } = useSourceChainTokenMappings(
    registryTokens,
    destChainBytes4,
    destMappingsQueryEnabled
  )

  const fallbackTokenConfig = TOKEN_CONFIGS[DEFAULT_NETWORK]

  /** Primary Anvil LockUnlock; Anvil1 uses VITE_EVM1_LOCK_UNLOCK_ADDRESS when set (QA / multi-EVM). */
  const evmLockForSource = useMemo((): Address => {
    const primary = (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || ZERO_LOCK) as Address
    const anvil1Lu = (import.meta.env.VITE_EVM1_LOCK_UNLOCK_ADDRESS || primary) as Address
    if (sourceChain === 'anvil1') return anvil1Lu
    return primary
  }, [sourceChain])

  const mergedFallbackTokenConfig = useMemo(() => {
    if (!fallbackTokenConfig) return undefined
    return { ...fallbackTokenConfig, lockUnlockAddress: evmLockForSource }
  }, [fallbackTokenConfig, evmLockForSource])

  // Only apply chain mapping filters once ALL queries are done.
  // Partial results would exclude tokens whose queries are still in flight.
  const readySourceMappings = isSourceEvm && !isSourceMappingsLoading ? sourceChainMappings : undefined
  const readyDestMappings =
    (isSourceTerra || isSourceSolana) &&
    (isDestEvmChain || isDestSolanaChain || (isSourceSolana && isDestCosmosChain)) &&
    !isDestMappingsLoading
      ? destChainMappings
      : undefined
  const transferTokens = useMemo(
    () =>
      buildTransferTokens(
        registryTokens,
        isSourceTerra,
        isSourceSolana,
        mergedFallbackTokenConfig,
        tokenlist ?? null,
        readySourceMappings,
        readyDestMappings,
        isSourceEvm && isSourceMappingsLoading,
      ),
    [
      registryTokens,
      isSourceTerra,
      isSourceSolana,
      mergedFallbackTokenConfig,
      tokenlist,
      readySourceMappings,
      readyDestMappings,
      isSourceEvm,
      isSourceMappingsLoading,
    ]
  )

  const readySourceDecimals = isSourceEvm && !isSourceMappingsLoading ? sourceChainDecimals : undefined
  const evmTokenConfig = useMemo(
    () =>
      getEvmTokenConfig(
        selectedTokenId,
        registryTokens,
        mergedFallbackTokenConfig,
        evmLockForSource,
        readySourceMappings,
        readySourceDecimals,
      ),
    [selectedTokenId, registryTokens, mergedFallbackTokenConfig, readySourceMappings, readySourceDecimals, evmLockForSource]
  )
  const tokenConfig = isSourceTerra || isSourceSolana ? undefined : evmTokenConfig
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

  const { data: tokenDestMappingAddr, isLoading: isTokenDestMappingLoading } = useTokenDestMapping(
    selectedTokenId || undefined,
    destChainBytes4,
    !!destChainBytes4 && (!!isDestEvmChain || isDestSolanaChain)
  )

  const destTokenAddr = useMemo(() => {
    if (destChainConfig?.type !== 'evm') return ''
    if (tokenDestMappingAddr) return tokenDestMappingAddr
    const reg = registryTokens?.find((t) => t.token === selectedTokenId)
    return reg?.evm_token_address ? bytes32ToAddress(reg.evm_token_address) : ''
  }, [destChainConfig?.type, tokenDestMappingAddr, registryTokens, selectedTokenId])

  /** Same cache as useTokenDestMapping — raw bytes32 for Solana TokenMapping PDA (incl. Solana→Terra). */
  const { data: solanaTokenDestMappingRaw } = useTokenDestMappingRaw(
    selectedTokenId || undefined,
    destChainBytes4,
    !!isSourceSolana && !!selectedTokenId && !!destChainBytes4,
  )

  /** 32-byte dest-chain token id for Solana `deposit_*` TokenMapping PDA (matches on-chain registry). */
  const solanaTokenMappingDest32 = useMemo(() => {
    if (!isSourceSolana) return null
    if (solanaTokenDestMappingRaw?.hex) {
      return hexToUint8Array(solanaTokenDestMappingRaw.hex)
    }
    if (destChainConfig?.type === 'evm' && destTokenAddr) {
      return hexToUint8Array(evmAddressToBytes32(destTokenAddr as `0x${string}`))
    }
    return null
  }, [isSourceSolana, solanaTokenDestMappingRaw, destChainConfig?.type, destTokenAddr])

  const solanaProgramIdStr =
    sourceChainConfig?.type === 'solana'
      ? getSolanaProgramIdString(sourceChainConfig)
      : null

  const sourceSolanaRpcUrls = useMemo(() => {
    if (sourceChainConfig?.type === 'solana') {
      return solanaRpcUrlsForBridgeChain(sourceChainConfig)
    }
    return [] as string[]
  }, [sourceChainConfig])
  const sourceSolanaRpcUrlsKey = sourceSolanaRpcUrls.join('|')

  const destChainBytesUint8 = useMemo(() => {
    if (!destChainBytes4) return null
    return bytes4HexToUint8Array(destChainBytes4)
  }, [destChainBytes4])

  const solanaMappingQueryEnabled =
    isSourceSolana &&
    sourceSolanaRpcUrls.length > 0 &&
    !!solanaProgramIdStr &&
    !!destChainBytesUint8 &&
    !!solanaTokenMappingDest32

  const { data: solanaLocalMint, isLoading: isSolanaLocalMintLoading } = useQuery({
    queryKey: [
      'solanaDepositLocalMint',
      sourceSolanaRpcUrlsKey,
      solanaProgramIdStr,
      destChainBytes4,
      solanaTokenMappingDest32,
    ],
    queryFn: async () => {
      const programId = new PublicKey(solanaProgramIdStr!)
      return withSolanaReadFallback(sourceSolanaRpcUrls, (connection) =>
        fetchTokenMappingLocalMint(
          connection,
          programId,
          destChainBytesUint8!,
          solanaTokenMappingDest32!,
        ),
      )
    },
    enabled: solanaMappingQueryEnabled,
  })

  const solanaDepositNative = useMemo(
    () => Boolean(solanaLocalMint && solanaLocalMint.equals(WSOL_MINT)),
    [solanaLocalMint],
  )
  const solanaDepositSpl = useMemo(
    () => Boolean(solanaLocalMint && !solanaLocalMint.equals(WSOL_MINT)),
    [solanaLocalMint],
  )

  const { data: solanaSplDecimals, isLoading: isSolanaSplDecimalsLoading } = useQuery({
    queryKey: ['solanaSplDecimals', sourceSolanaRpcUrlsKey, solanaLocalMint?.toBase58()],
    queryFn: async () =>
      withSolanaReadFallback(sourceSolanaRpcUrls, (connection) =>
        fetchSplMintDecimals(connection, solanaLocalMint!),
      ),
    enabled: !!solanaDepositSpl && sourceSolanaRpcUrls.length > 0 && !!solanaLocalMint,
  })

  const { data: solanaSourceBalance } = useQuery({
    queryKey: [
      'solanaSourceBalance',
      sourceSolanaRpcUrlsKey,
      solanaAddress,
      solanaDepositNative,
      solanaLocalMint?.toBase58(),
    ],
    queryFn: async () =>
      withSolanaReadFallback(sourceSolanaRpcUrls, async (connection) => {
        const addr = new PublicKey(solanaAddress!)
        if (solanaDepositNative) {
          return bigintFromBaseUnitsString(await connection.getBalance(addr))
        }
        const mint = solanaLocalMint!
        const mintInfo = await connection.getAccountInfo(mint)
        if (!mintInfo) return 0n
        const { getAssociatedTokenAddressSync } = await import('@solana/spl-token')
        const ata = getAssociatedTokenAddressSync(mint, addr, false, mintInfo.owner)
        try {
          const bal = await connection.getTokenAccountBalance(ata)
          return bigintFromBaseUnitsString(bal.value.amount)
        } catch {
          return 0n
        }
      }),
    enabled:
      !!isSourceSolana &&
      sourceSolanaRpcUrls.length > 0 &&
      !!solanaAddress &&
      !!solanaLocalMint &&
      (solanaDepositNative || solanaDepositSpl),
  })

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

  // Resolve terra1 address for CW20 tokens (e.g. TDEC); native uluna uses bank balance
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
    !isSourceTerra && !isSourceSolana && !!tokenConfig
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
      sourceChainConfig: config,
    }
  }, [sourceChain])

  /** EVM → Solana: TokenRegistry stores SPL mint as bytes32; validators need base58 mint. */
  const { data: evmToSolanaDestMint, isLoading: isEvmToSolanaDestLoading } = useQuery({
    queryKey: ['evmToSolanaDestMint', sourceBridgeConfig?.bridgeAddress, tokenConfig?.address, destChainBytes4],
    queryFn: async () => {
      if (!sourceBridgeConfig || !tokenConfig?.address || !destChainBytes4) return null
      const client = getEvmClient(sourceBridgeConfig.sourceChainConfig)
      const dt = await getDestToken(
        client,
        sourceBridgeConfig.bridgeAddress,
        tokenConfig.address as Address,
        destChainBytes4 as Hex
      )
      if (!dt || dt === ('0x' + '0'.repeat(64))) return null
      return bytes32ToSolanaAddress(dt as `0x${string}`)
    },
    enabled:
      direction === 'evm-to-solana' &&
      !!sourceBridgeConfig &&
      !!tokenConfig?.address &&
      !!destChainBytes4,
  })

  const solanaDestTokenIdForRoute = useMemo(() => {
    if (destChainConfig?.type !== 'solana') return undefined
    if (direction === 'terra-to-solana') return tokenDestMappingAddr ?? undefined
    if (direction === 'evm-to-solana') return evmToSolanaDestMint ?? undefined
    return undefined
  }, [destChainConfig?.type, direction, tokenDestMappingAddr, evmToSolanaDestMint])

  const {
    deposit: evmDeposit,
    status: evmStatus,
    approvalTxHash: evmApprovalTxHash,
    depositTxHash,
    error: evmError,
    reset: resetEvm,
    tokenBalance,
  } = useBridgeDeposit(
    tokenConfig && !isSourceSolana
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

  const effectiveTokenBalance = isSourceSolana ? solanaSourceBalance : tokenBalance

  const { lock: terraLock, status: terraStatus, txHash: terraTxHash, error: terraError, reset: resetTerra } = useTerraDeposit()

  const isWalletConnected = isSourceTerra ? isTerraConnected : isSourceSolana ? isSolanaConnected : isEvmConnected

  // Auto-fill recipient from connected wallet on the destination side
  const recipientAddr = useMemo(() => {
    if (recipient) return recipient
    if (isDestSolana) return solanaAddress ?? ''
    if (isDestEvm) return evmAddress ?? ''
    return terraAddress ?? ''
  }, [recipient, isDestEvm, isDestSolana, evmAddress, terraAddress, solanaAddress])

  const isSubmitting =
    evmStatus === 'switching-chain' ||
    evmStatus === 'checking-allowance' ||
    evmStatus === 'approving' ||
    evmStatus === 'waiting-approval' ||
    evmStatus === 'depositing' ||
    evmStatus === 'waiting-deposit' ||
    terraStatus === 'locking' ||
    solanaStep === 'building' ||
    solanaStep === 'signing' ||
    solanaStep === 'confirming'

  const registryDecimalsFallback = useMemo(
    () => getTerraTokenDecimals(selectedTokenId, registryTokens),
    [selectedTokenId, registryTokens]
  )
  const amountDecimals = isSourceTerra
    ? terraDecimals
    : isSourceSolana
      ? solanaDepositSpl
        ? (solanaSplDecimals ?? registryDecimalsFallback)
        : 9
      : (tokenConfig?.decimals ?? DECIMALS.LUNC)

  const receiveAmount = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return '0'
    try {
      const gross = parseAmountAsBigInt(amount, amountDecimals)
      const feeBps = BigInt(Math.round(BRIDGE_CONFIG.feePercent * 100))
      const net = gross - (gross * feeBps) / 10000n
      return formatAmount(net, amountDecimals, 6)
    } catch {
      return '0'
    }
  }, [amount, amountDecimals])

  const destDecimals = isDestEvmChain
    ? (destChainDecimals?.[selectedTokenId]
        ?? registryTokens?.find((t) => t.token === selectedTokenId)?.evm_decimals
        ?? 18)
    : terraDecimals

  const toSourceUnits = useCallback(
    (destBaseUnits: bigint) => {
      if (amountDecimals >= destDecimals) {
        const exp = amountDecimals - destDecimals
        return destBaseUnits * pow10BigInt(exp)
      }
      const exp = destDecimals - amountDecimals
      const result = destBaseUnits / pow10BigInt(exp)
      if (result === 0n && destBaseUnits > 0n) return 1n
      return result
    },
    [amountDecimals, destDecimals]
  )

  const bridgeFeeBps = useMemo(
    () => BigInt(Math.round(BRIDGE_CONFIG.feePercent * 100)),
    []
  )

  const { displayMaxLabel, displayBridgeMax, effectiveMinInSrc, effectiveMaxInSrc } = useMemo(() => {
    const srcDecimals = amountDecimals
    let balanceStr: string | undefined
    if (isSourceTerra) {
      balanceStr = terraSourceBalance
    } else if (isSourceSolana && effectiveTokenBalance !== undefined) {
      balanceStr = effectiveTokenBalance.toString()
    } else if (tokenBalance !== undefined && tokenConfig) {
      balanceStr = tokenBalance.toString()
    }
    const balance = balanceStr ? bigintFromBaseUnitsString(balanceStr) : 0n

    let maxTransferInSrc: bigint | null = null
    if (destTokenDetails?.maxTransfer) {
      const raw = bigintFromBaseUnitsString(destTokenDetails.maxTransfer)
      if (raw > 0n) maxTransferInSrc = toSourceUnits(raw)
    }
    let bridgeRemainingInSrc: bigint | null = null
    if (destTokenDetails?.withdrawRateLimit?.remainingAmount) {
      const raw = bigintFromBaseUnitsString(destTokenDetails.withdrawRateLimit.remainingAmount)
      if (raw > 0n) bridgeRemainingInSrc = toSourceUnits(raw)
    }

    let effectiveMax = balance
    if (maxTransferInSrc != null && maxTransferInSrc > 0n)
      effectiveMax = effectiveMax < maxTransferInSrc ? effectiveMax : maxTransferInSrc
    if (bridgeRemainingInSrc != null && bridgeRemainingInSrc > 0n)
      effectiveMax = effectiveMax < bridgeRemainingInSrc ? effectiveMax : bridgeRemainingInSrc

    let effectiveMin: bigint | null = null
    const minCandidates: bigint[] = []
    if (destTokenDetails?.minTransfer) {
      const raw = bigintFromBaseUnitsString(destTokenDetails.minTransfer)
      if (raw > 0n) minCandidates.push(toSourceUnits(raw))
    }
    if (destTokenDetails?.withdrawRateLimit?.minPerTransaction) {
      const raw = bigintFromBaseUnitsString(
        destTokenDetails.withdrawRateLimit.minPerTransaction,
      )
      if (raw > 0n) minCandidates.push(toSourceUnits(raw))
    }
    if (minCandidates.length) {
      effectiveMin = minCandidates.reduce((a, b) => (a > b ? a : b))
    }

    const bridgeLimit = bridgeRemainingInSrc ?? maxTransferInSrc
    return {
      displayMaxLabel:
        effectiveMax > 0n ? formatCompact(effectiveMax.toString(), srcDecimals) : undefined,
      displayBridgeMax:
        bridgeLimit != null && bridgeLimit > 0n ? formatCompact(bridgeLimit.toString(), srcDecimals) : undefined,
      effectiveMinInSrc: effectiveMin,
      effectiveMaxInSrc: effectiveMax,
    }
  }, [
    isSourceTerra,
    isSourceSolana,
    terraSourceBalance,
    effectiveTokenBalance,
    tokenBalance,
    tokenConfig,
    destTokenDetails?.maxTransfer,
    destTokenDetails?.minTransfer,
    destTokenDetails?.withdrawRateLimit?.minPerTransaction,
    destTokenDetails?.withdrawRateLimit?.remainingAmount,
    amountDecimals,
    toSourceUnits,
  ])

  /** Smallest send amount (gross) so net after bridge fee is at least the destination minimum (matches validation). */
  const { minSendGrossInSrc, displayMinLabel } = useMemo(() => {
    if (effectiveMinInSrc == null || effectiveMinInSrc <= 0n) {
      return { minSendGrossInSrc: null as bigint | null, displayMinLabel: undefined as string | undefined }
    }
    const gross = minGrossForMinNet(effectiveMinInSrc, bridgeFeeBps)
    return {
      minSendGrossInSrc: gross,
      // Full precision: compact sigfigs round e.g. 1.005025 → "1.005", so typing the label would fail isBelowMin.
      displayMinLabel: formatAmountForNumberInput(gross.toString(), amountDecimals, amountDecimals),
    }
  }, [effectiveMinInSrc, bridgeFeeBps, amountDecimals])

  const isBelowMin = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return false
    const gross = parseAmountAsBigInt(amount, amountDecimals)
    if (effectiveMinInSrc != null && effectiveMinInSrc > 0n) {
      const net = gross - (gross * bridgeFeeBps) / 10000n
      return net < effectiveMinInSrc
    }
    return false
  }, [amount, amountDecimals, effectiveMinInSrc, bridgeFeeBps])

  const isAboveMax = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return false
    if (effectiveMaxInSrc <= 0n) {
      // Distinguish "balance not loaded" (don't block) from "balance is 0" (block)
      return (
        isSourceTerra ||
        (isSourceSolana && effectiveTokenBalance !== undefined) ||
        (tokenBalance !== undefined && !!tokenConfig)
      )
    }
    const parsed = parseAmountAsBigInt(amount, amountDecimals)
    return parsed > effectiveMaxInSrc
  }, [
    amount,
    amountDecimals,
    effectiveMaxInSrc,
    isSourceTerra,
    isSourceSolana,
    effectiveTokenBalance,
    tokenBalance,
    tokenConfig,
  ])

  const handleMax = useCallback(() => {
    const srcDecimals = amountDecimals
    let balanceStr: string
    if (isSourceTerra) {
      balanceStr = terraSourceBalance
    } else if (isSourceSolana && effectiveTokenBalance !== undefined) {
      balanceStr = effectiveTokenBalance.toString()
    } else if (tokenBalance !== undefined && tokenConfig) {
      balanceStr = tokenBalance.toString()
    } else {
      return
    }
    const balance = bigintFromBaseUnitsString(balanceStr)

    let maxTransferInSrc: bigint | null = null
    if (destTokenDetails?.maxTransfer) {
      const raw = bigintFromBaseUnitsString(destTokenDetails.maxTransfer)
      if (raw > 0n) maxTransferInSrc = toSourceUnits(raw)
    }
    let bridgeRemainingInSrc: bigint | null = null
    if (destTokenDetails?.withdrawRateLimit?.remainingAmount) {
      const raw = bigintFromBaseUnitsString(destTokenDetails.withdrawRateLimit.remainingAmount)
      if (raw > 0n) bridgeRemainingInSrc = toSourceUnits(raw)
    }

    let effectiveMax = balance
    if (maxTransferInSrc != null && maxTransferInSrc > 0n)
      effectiveMax = effectiveMax < maxTransferInSrc ? effectiveMax : maxTransferInSrc
    if (bridgeRemainingInSrc != null && bridgeRemainingInSrc > 0n)
      effectiveMax = effectiveMax < bridgeRemainingInSrc ? effectiveMax : bridgeRemainingInSrc
    setAmount(formatAmountForNumberInput(effectiveMax, srcDecimals))
  }, [
    isSourceTerra,
    isSourceSolana,
    terraSourceBalance,
    effectiveTokenBalance,
    tokenBalance,
    tokenConfig,
    destTokenDetails?.maxTransfer,
    destTokenDetails?.withdrawRateLimit?.remainingAmount,
    amountDecimals,
    toSourceUnits,
  ])

  const handleMin = useCallback(() => {
    if (minSendGrossInSrc == null || minSendGrossInSrc <= 0n) return
    setAmount(formatAmountForNumberInput(minSendGrossInSrc, amountDecimals, amountDecimals))
  }, [minSendGrossInSrc, amountDecimals])

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

        // Get source chain bytes4 for the record
        const tier = DEFAULT_NETWORK as NetworkTier
        const chains = BRIDGE_CHAINS[tier]
        const srcChainConfig = chains[frozenSource]
        const destChainConfig = chains[frozenDest]

        // Query the destination token from TokenRegistry BEFORE computing the hash.
        // The EVM contract hashes with destToken (not the source ERC20 address), so we
        // must use the same value here for the hashes to match across chains.
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

        // Compute transfer hash if we have the nonce.
        // CRITICAL: use the destination token (destTokenBytes32) for the token field,
        // matching the EVM contract's HashLib.computeXchainHashId which hashes with
        // tokenRegistry.getDestToken(). The Deposit event's `token` field is the
        // SOURCE ERC20 address and must NOT be used here.
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
                bigintFromBaseUnitsString(depositAmount),
                bigintFromBaseUnitsString(depositNonce)
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
          // For EVM->Terra: store Terra denom/CW20 bech32 — never an 0x EVM address (fallback row
          // while mappings load), or withdraw_submit hashes keccak(ascii "0x...") and mismatches EVM.
          // For other directions: store the dest token address as-is.
          destTokenId: frozenDirection === 'evm-to-terra'
            ? (resolveTerraDestTokenIdForRecord(selectedTokenId, destTokenBytes32, tokenlist ?? null) ||
              (selectedTokenId && !selectedTokenId.startsWith('0x') ? selectedTokenId : '') ||
              'uluna')
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
  }, [
    evmStatus,
    depositTxHash,
    evmError,
    resetEvm,
    sourceChain,
    destChain,
    amount,
    amountDecimals,
    recordTransfer,
    direction,
    navigate,
    publicClient,
    evmAddress,
    tokenConfig,
    recipientAddr,
    selectedTokenId,
    tokenlist,
  ])

  // Terra deposit success: parse receipt for nonce, use canonical xchain_hash_id + dest_token_address
  // from the Terra contract's own event attributes (not recomputed by the frontend).
  // Uses frozenChainsRef for the same reason as the EVM handler above.
  useEffect(() => {
    if (terraStatus === 'success' && terraTxHash) {
      setTxHash(terraTxHash)

      const frozen = frozenChainsRef.current
      const frozenDest = frozen?.destChain ?? destChain
      const frozenSource = frozen?.sourceChain ?? sourceChain
      const recordDirection = frozen?.direction ?? direction

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

        let destAccountHex: string
        if (recipientAddr.startsWith('0x') && recipientAddr.length === 42) {
          destAccountHex = evmAddressToBytes32(recipientAddr as `0x${string}`)
        } else if (recordDirection === 'terra-to-solana') {
          try {
            destAccountHex = solanaAddressToBytes32(recipientAddr)
          } catch {
            destAccountHex = recipientAddr
          }
        } else {
          destAccountHex = recipientAddr
        }

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
          // try destTokenAddr / mapping from useTokenDestMapping, then registry
          if (recordDirection === 'terra-to-solana' && tokenDestMappingAddr) {
            try {
              destTokenB32 = solanaAddressToBytes32(tokenDestMappingAddr)
            } catch {
              /* keep undefined */
            }
          } else if (destTokenAddr) {
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
            const destAccB32 =
              destAccountHex.startsWith('0x') && destAccountHex.length === 66
                ? destAccountHex
                : recordDirection === 'terra-to-solana'
                  ? solanaAddressToBytes32(recipientAddr)
                  : evmAddressToBytes32(recipientAddr as `0x${string}`)
            const tokenB32 =
              destTokenB32 && destTokenB32.length === 66
                ? destTokenB32
                : recordDirection === 'terra-to-solana' && tokenDestMappingAddr
                  ? solanaAddressToBytes32(tokenDestMappingAddr)
                  : destTokenAddr
                    ? evmAddressToBytes32(destTokenAddr as `0x${string}`)
                    : '0x' + '0'.repeat(64)
            xchainHashId = computeXchainHashId(
              srcChainB32 as `0x${string}`,
              destChainB32 as `0x${string}`,
              srcAccB32 as `0x${string}`,
              destAccB32 as `0x${string}`,
              tokenB32 as `0x${string}`,
              bigintFromBaseUnitsString(netAmount),
              bigintFromBaseUnitsString(depositNonce)
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
          direction: recordDirection === 'terra-to-solana' ? 'terra-to-solana' : 'terra-to-evm',
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
          token: (parsed?.token ?? selectedTokenId) || 'uluna',
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
  }, [
    terraStatus,
    terraTxHash,
    terraError,
    resetTerra,
    navigate,
    destChain,
    sourceChain,
    amount,
    terraDecimals,
    recordTransfer,
    terraAddress,
    recipientAddr,
    selectedTokenId,
    registryTokens,
    destTokenAddr,
    direction,
    tokenDestMappingAddr,
  ])

  // Solana deposit success: V2 xchain hash in URL + store (status page expects 0x…, not Solana tx sig)
  useEffect(() => {
    if (solanaStep !== 'confirmed' || !solanaTxSig || confirmedDepositNonce == null) return

    const frozen = frozenChainsRef.current
    if (!frozen) return

    const destTokHex =
      solanaTokenDestMappingRaw?.hex ||
      (destTokenAddr ? evmAddressToBytes32(destTokenAddr as `0x${string}`) : undefined)

    let destAccStored = recipientAddr
    if (recipientAddr.startsWith('0x') && recipientAddr.length === 42) {
      destAccStored = evmAddressToBytes32(recipientAddr as `0x${string}`)
    } else if (recipientAddr && !recipientAddr.startsWith('0x')) {
      try {
        destAccStored = solanaAddressToBytes32(recipientAddr)
      } catch {
        /* keep raw */
      }
    }

    const gross = parseAmountAsBigInt(amount, amountDecimals)
    const feeBps = BigInt(Math.round(BRIDGE_CONFIG.feePercent * 100))
    const fee = (gross * feeBps) / 10000n
    const netAmount = gross - fee
    const netAmountStr = netAmount.toString()

    const tier = DEFAULT_NETWORK as NetworkTier
    const srcCfg = BRIDGE_CHAINS[tier][frozen.sourceChain]
    const destCfg = BRIDGE_CHAINS[tier][frozen.destChain]
    const srcBytes4 = frozen.sourceChainIdBytes4 ?? srcCfg?.bytes4ChainId
    const destBytes4 = destCfg?.bytes4ChainId

    const tokenSymbolRecorded =
      isSourceSolana || isSourceTerra
        ? terraDisplay.displayLabel ||
          transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
          getTokenDisplaySymbol(selectedTokenId || 'uluna')
        : evmSourceDisplay.displayLabel ||
          tokenConfig?.symbol ||
          transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
          '—'

    let xchainHashId: `0x${string}` | undefined
    try {
      if (
        srcBytes4 &&
        destBytes4 &&
        destTokHex &&
        destTokHex.length === 66 &&
        solanaAddress
      ) {
        const srcChainBytes = bytes4HexToUint8Array(srcBytes4)
        const destChainBytes = bytes4HexToUint8Array(destBytes4)
        const tokenBytes = hexToBytes(destTokHex as Hex)
        const srcAccBytes = hexToBytes(solanaAddressToBytes32(solanaAddress) as Hex)
        let destAccHex: `0x${string}`
        if (recipientAddr.startsWith('0x') && recipientAddr.length === 42) {
          destAccHex = evmAddressToBytes32(recipientAddr as `0x${string}`)
        } else if (recipientAddr.startsWith('terra1')) {
          destAccHex = terraAddressToBytes32(recipientAddr)
        } else {
          destAccHex = solanaAddressToBytes32(recipientAddr)
        }
        const destAccBytes = hexToBytes(destAccHex)

        xchainHashId = computeXchainHashIdFromBytes(
          srcChainBytes,
          destChainBytes,
          srcAccBytes,
          destAccBytes,
          tokenBytes,
          netAmount,
          bigintFromBaseUnitsString(confirmedDepositNonce),
        )
      }
    } catch (e) {
      console.warn('[TransferForm:solana] Failed to compute xchainHashId:', e)
    }

    recordTransfer({
      sourceChain: frozen.sourceChain,
      destChain: frozen.destChain,
      direction: frozen.direction,
      type: 'deposit',
      token: selectedTokenId || tokenSymbolRecorded || 'SOL',
      amount: netAmountStr,
      status: 'confirmed',
      srcAccount: solanaAddress ? solanaAddressToBytes32(solanaAddress) : '',
      txHash: solanaTxSig,
      lifecycle: 'deposited',
      sourceChainIdBytes4: sourceChainConfig?.bytes4ChainId,
      depositNonce: confirmedDepositNonce,
      destAccount: destAccStored,
      destToken: destTokHex,
      ...(xchainHashId ? { xchainHashId } : {}),
      srcDecimals: amountDecimals,
      tokenSymbol: tokenSymbolRecorded,
    })
    frozenChainsRef.current = null
    resetSolana()
    navigate(xchainHashId ? `/transfer/${xchainHashId}` : `/transfer/${solanaTxSig}`)
  }, [
    solanaStep,
    solanaTxSig,
    confirmedDepositNonce,
    amount,
    amountDecimals,
    solanaAddress,
    sourceChainConfig,
    recipientAddr,
    destTokenAddr,
    solanaTokenDestMappingRaw,
    recordTransfer,
    resetSolana,
    navigate,
    selectedTokenId,
    isSourceSolana,
    isSourceTerra,
    terraDisplay.displayLabel,
    evmSourceDisplay.displayLabel,
    tokenConfig?.symbol,
    transferTokens,
  ])

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
    if (destInfo?.type === 'cosmos' && sourceInfo?.type === 'cosmos') return true
    if (destInfo?.type === 'solana' && sourceInfo?.type === 'solana') return true
    return false
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

    if (!isWalletConnected) {
      setError(`Connect your ${walletLabel} wallet to continue`)
      return
    }
    if (!amount || !isValidAmount(amount)) {
      setError('Enter a valid amount greater than zero')
      return
    }
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

    if (isSourceSolana) {
      // Solana → EVM or Solana → Terra: deposit on Solana bridge
      if (!recipientAddr) {
        setError('Please provide a recipient address or connect your destination wallet')
        frozenChainsRef.current = null
        return
      }
      const sourceConfig = sourceChainConfig
      const solanaProgramIdStr =
        sourceConfig?.type === 'solana' ? getSolanaProgramIdString(sourceConfig) : null
      const depositRpcUrls =
        sourceConfig?.type === 'solana' ? solanaRpcUrlsForBridgeChain(sourceConfig) : []
      if (depositRpcUrls.length === 0 || !solanaProgramIdStr) {
        setError(`Missing Solana RPC or program ID config for source chain: ${sourceChain}`)
        frozenChainsRef.current = null
        return
      }
      const destV2Bytes4 = destConfig?.bytes4ChainId
      if (!destV2Bytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        frozenChainsRef.current = null
        return
      }

      try {
        const connection = await pickSolanaConnection(depositRpcUrls)
        const programId = new PublicKey(solanaProgramIdStr)
        const depositNonce = await fetchDepositNonce(connection, programId)

        // Encode destination parameters as bytes
        const destChainBytes = new Uint8Array(4)
        const destChainNum = parseInt(destV2Bytes4, 16)
        destChainBytes[0] = (destChainNum >> 24) & 0xff
        destChainBytes[1] = (destChainNum >> 16) & 0xff
        destChainBytes[2] = (destChainNum >> 8) & 0xff
        destChainBytes[3] = destChainNum & 0xff

        // Encode dest account as 32 bytes
        const destAccountBytes = new Uint8Array(32)
        if (recipientAddr.startsWith('0x')) {
          const hex = recipientAddr.slice(2)
          for (let i = 0; i < 20 && i * 2 < hex.length; i++) {
            destAccountBytes[12 + i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16)
          }
        } else {
          // Solana base58 or Terra address — encode as raw bytes
          try {
            const pk = new PublicKey(recipientAddr)
            destAccountBytes.set(pk.toBytes())
          } catch {
            setError('Invalid recipient address')
            frozenChainsRef.current = null
            return
          }
        }

        if (!solanaTokenMappingDest32) {
          setError(
            'Could not resolve destination token mapping for this route. Ensure the token is registered for the destination chain (Terra LCD mapping or EVM dest address).'
          )
          frozenChainsRef.current = null
          return
        }

        if (solanaMappingQueryEnabled && solanaLocalMint === null) {
          setError(
            'This token is not registered on the Solana bridge for the selected destination chain.',
          )
          frozenChainsRef.current = null
          return
        }

        const amountBaseUnits = parseAmountAsBigInt(amount, amountDecimals)

        await solanaDeposit({
          rpcUrls: depositRpcUrls,
          programId: solanaProgramIdStr,
          destChain: destChainBytes,
          destAccount: destAccountBytes,
          tokenMappingDestToken: solanaTokenMappingDest32,
          amount: amountBaseUnits,
          depositNonce,
          splMint:
            solanaDepositSpl && solanaLocalMint ? solanaLocalMint.toBase58() : undefined,
        })
      } catch (err) {
        setError(err instanceof Error ? err.message : formatSolanaWalletError(err))
        frozenChainsRef.current = null
        resetSolana()
      }
      return
    }

    if (direction === 'terra-to-solana') {
      if (!recipientAddr?.trim()) {
        setError('Please provide a Solana recipient address or connect your Solana wallet')
        frozenChainsRef.current = null
        return
      }
      try {
        new PublicKey(recipientAddr)
      } catch {
        setError('Please provide a Solana recipient address or connect your Solana wallet')
        frozenChainsRef.current = null
        return
      }
      const v2Bytes4Sol = destConfig?.bytes4ChainId
      if (!v2Bytes4Sol) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        frozenChainsRef.current = null
        return
      }
      const destChainIdSol = parseInt(v2Bytes4Sol, 16)
      const amountMicroSol = parseAmount(amount, terraDecimals)
      const selectedRegSol = registryTokens?.find((t) => t.token === selectedTokenId)
      const isNativeSol = selectedRegSol?.is_native ?? (selectedTokenId === 'uluna')
      await terraLock({
        amountMicro: amountMicroSol,
        destChainId: destChainIdSol,
        recipientSolana: recipientAddr,
        tokenId: selectedTokenId || 'uluna',
        isNative: isNativeSol,
        srcDecimals: terraDecimals,
        transferDirection: 'terra-to-solana',
        destChainKey: destChain,
        tokenSymbol:
          terraDisplay.displayLabel ||
          transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
          getTokenDisplaySymbol(selectedTokenId || 'uluna'),
      })
      return
    }

    if (direction === 'terra-to-evm') {
      // Terra → EVM: deposit on Terra bridge (V2)
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        frozenChainsRef.current = null
        return
      }
      const v2Bytes4 = destConfig?.bytes4ChainId
      if (!v2Bytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        frozenChainsRef.current = null
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
        destChainKey: destChain,
        tokenSymbol:
          terraDisplay.displayLabel ||
          transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
          getTokenDisplaySymbol(selectedTokenId || 'uluna'),
      })
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
      const destChainBytes4 = destConfig?.bytes4ChainId as Hex | undefined
      if (!destChainBytes4) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        frozenChainsRef.current = null
        return
      }
      const destAccount = encodeTerraAddress(recipientAddr)
      await evmDeposit(amount, destChainBytes4, destAccount, tokenConfig.decimals)
    } else if (direction === 'evm-to-solana') {
      // EVM → Solana: same Bridge.depositERC20 path; dest account is pubkey as bytes32
      if (!recipientAddr?.trim()) {
        setError('Please provide a Solana recipient address or connect your Solana wallet')
        frozenChainsRef.current = null
        return
      }
      try {
        new PublicKey(recipientAddr)
      } catch {
        setError('Please provide a Solana recipient address or connect your Solana wallet')
        frozenChainsRef.current = null
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        frozenChainsRef.current = null
        return
      }
      const destChainBytes4Sol = destConfig?.bytes4ChainId as Hex | undefined
      if (!destChainBytes4Sol) {
        setError(`Missing V2 bytes4 chain ID config for destination chain: ${destChain}`)
        frozenChainsRef.current = null
        return
      }
      const destAccountSol = solanaAddressToBytes32(recipientAddr) as Hex
      await evmDeposit(amount, destChainBytes4Sol, destAccountSol, tokenConfig.decimals)
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
    ? formatAmount(terraSourceBalance, terraDecimals)
    : isSourceSolana && effectiveTokenBalance !== undefined
    ? formatAmount(effectiveTokenBalance.toString(), amountDecimals)
    : tokenBalance !== undefined && tokenConfig
    ? formatAmount(tokenBalance.toString(), tokenConfig.decimals)
    : undefined

  // Use onchain/tokenlist display hooks for symbol (not raw address)
  const selectedSymbol =
    isSourceTerra || isSourceSolana
      ? terraDisplay.displayLabel ||
        transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
        getTokenDisplaySymbol(selectedTokenId || 'uluna')
      : evmSourceDisplay.displayLabel ||
        tokenConfig?.symbol ||
        transferTokens.find((t) => t.id === selectedTokenId)?.symbol ||
        '—'
  const walletLabel = isSourceTerra ? 'Terra' : isSourceSolana ? 'Solana' : 'EVM'

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

  const isTokenInfoLoading =
    (isSourceEvm && (isRegistryLoading || isSourceMappingsLoading)) ||
    (isSourceTerra && (isRegistryLoading || isDestMappingsLoading)) ||
    (isSourceSolana && (isRegistryLoading || isDestMappingsLoading)) ||
    (direction === 'evm-to-solana' && isEvmToSolanaDestLoading) ||
    (isSourceSolana &&
      (isSolanaLocalMintLoading || (solanaDepositSpl && solanaLocalMint && isSolanaSplDecimalsLoading)))
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
      !isTokenInfoLoading &&
      (!isSourceSolana || !isSolanaLocalMintLoading) &&
      (direction !== 'evm-to-solana' || !isEvmToSolanaDestLoading) &&
      (direction !== 'terra-to-solana' || !isTokenDestMappingLoading),
    tokenLabel: selectedSymbol,
    sourceChainConfig,
    destChainConfig,
    sourceTokenAddress: isSourceSolana ? solanaLocalMint?.toBase58() : tokenConfig?.address,
    sourceMappingAddress: readySourceMappings?.[selectedTokenId],
    destTokenAddress: destChainConfig?.type === 'evm' ? (destTokenAddr || undefined) : undefined,
    destMappingAddress: destChainConfig?.type === 'evm' ? (tokenDestMappingAddr || undefined) : undefined,
    destTokenId:
      destChainConfig?.type === 'cosmos'
        ? (terraCw20Address ?? selectedTokenId) || undefined
        : destChainConfig?.type === 'solana'
          ? solanaDestTokenIdForRoute
          : undefined,
    solanaMappingSourceBytes4:
      destChainConfig?.type === 'solana' ? sourceChainBytes4 : undefined,
    solanaMappingEvmTokenAddress:
      direction === 'evm-to-solana' ? tokenConfig?.address : undefined,
    solanaMappingTerraTokenId:
      direction === 'terra-to-solana' ? selectedTokenId || undefined : undefined,
  })
  const solanaMappingGuardError =
    isSourceSolana &&
    !isSolanaLocalMintLoading &&
    solanaMappingQueryEnabled &&
    solanaLocalMint === null
      ? 'This token is not registered on the Solana bridge for the selected destination chain.'
      : null

  const submitGuardError =
    solanaMappingGuardError ||
    (!isTokenInfoLoading && !isRouteValidationLoading && !isRouteValid
      ? routeValidationError
      : null)

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
    : evmStatus === 'waiting-approval'
    ? 'Confirming approval…'
    : evmStatus === 'waiting-deposit'
    ? 'Confirming deposit…'
    : isSubmitting
    ? 'Processing...'
    : direction === 'terra-to-evm' || direction === 'terra-to-solana'
    ? 'Bridge from Terra'
    : direction === 'solana-to-evm' || direction === 'solana-to-terra'
    ? 'Bridge from Solana'
    : direction === 'evm-to-evm'
    ? 'Bridge EVM to EVM'
    : direction === 'evm-to-solana'
    ? 'Bridge EVM to Solana'
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
          <p className="text-red-300 text-sm break-words whitespace-pre-wrap">{error}</p>
          <button type="button" onClick={() => setError(null)} className="text-red-200 text-xs mt-2 underline underline-offset-2">
            Dismiss
          </button>
        </div>
      )}
      {!error &&
        !txHash &&
        (evmStatus === 'waiting-approval' || evmStatus === 'waiting-deposit') && (
          <div className="border-2 border-cyan-700/70 bg-cyan-950/30 p-3 shadow-[2px_2px_0_#000]">
            <p className="text-cyan-200 text-xs font-semibold uppercase tracking-wide">
              {evmStatus === 'waiting-approval' ? 'Waiting for approval' : 'Waiting for deposit'}
            </p>
            <p className="text-cyan-100/85 text-sm mt-2">
              Your wallet submitted a transaction. Confirmation can take from a few seconds to several minutes depending on
              the network. If nothing appears after about two minutes, the transaction may have been dropped—check the
              explorer link below, your wallet activity, gas settings, and RPC.
            </p>
            {evmStatus === 'waiting-approval' && evmApprovalTxHash && (
              <p className="mt-2 text-xs text-cyan-300/90">
                <span className="font-mono break-all">{evmApprovalTxHash}</span>
                {getExplorerTxUrl(sourceChain, evmApprovalTxHash) ? (
                  <>
                    {' '}
                    <a
                      href={getExplorerTxUrl(sourceChain, evmApprovalTxHash)}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-cyan-400 underline underline-offset-2 hover:text-cyan-300"
                    >
                      View on explorer →
                    </a>
                  </>
                ) : null}
              </p>
            )}
            {evmStatus === 'waiting-deposit' && depositTxHash && (
              <p className="mt-2 text-xs text-cyan-300/90">
                <span className="font-mono break-all">{depositTxHash}</span>
                {getExplorerTxUrl(sourceChain, depositTxHash) ? (
                  <>
                    {' '}
                    <a
                      href={getExplorerTxUrl(sourceChain, depositTxHash)}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-cyan-400 underline underline-offset-2 hover:text-cyan-300"
                    >
                      View on explorer →
                    </a>
                  </>
                ) : null}
              </p>
            )}
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
      {!error &&
        !submitGuardError &&
        isSourceSolana &&
        sourceChain === 'solana-localnet' &&
        solanaWalletType?.toLowerCase() === 'phantom' && (
          <div className="bg-amber-900/30 border-2 border-amber-700 p-3">
            <p className="text-amber-200 text-sm font-semibold">Phantom and Solana Localnet</p>
            <p className="text-amber-100/90 text-sm mt-1">
              Phantom often cannot open a sign popup for a local validator; the extension may report that signing is not
              supported on local networks. Use Solflare or Backpack for manual UI testing, or rely on automated E2E for
              Solana Localnet.
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
        windowActive={destTokenDetails?.withdrawRateLimit?.windowActive}
        disabled={isSubmitting}
      />
      <AmountInput
        value={amount}
        onChange={setAmount}
        onMin={minSendGrossInSrc != null && minSendGrossInSrc > 0n ? handleMin : undefined}
        onMax={
          isSourceTerra && terraSourceBalance
            ? handleMax
            : isSourceSolana && effectiveTokenBalance !== undefined
            ? handleMax
            : tokenBalance !== undefined && tokenConfig
            ? handleMax
            : undefined
        }
        tokens={transferTokens}
        selectedTokenId={selectedTokenId}
        onTokenChange={setSelectedTokenId}
        symbol={selectedSymbol}
        sourceChainConfigOrRpcUrl={
          !isSourceTerra && !isSourceSolana && sourceChainConfig?.type === 'evm' ? sourceChainConfig : undefined
        }
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
          if (isDestSolana) {
            if (solanaAddress) setRecipient(solanaAddress)
            else setShowSolanaModal(true)
          } else if (isDestEvm) {
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
