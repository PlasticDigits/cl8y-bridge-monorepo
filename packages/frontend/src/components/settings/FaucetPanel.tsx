import { useState, useEffect, useCallback, useMemo } from 'react'
import {
  useAccount,
  useWriteContract,
  useWaitForTransactionReceipt,
  useSwitchChain,
} from 'wagmi'
import { useQuery } from '@tanstack/react-query'
import { Card, Spinner } from '../ui'
import { useWalletStore } from '../../stores/wallet'
import { executeContractWithCoins } from '../../services/terra'
import { queryContract } from '../../services/lcdClient'
import { getEvmClient } from '../../services/evmClient'
import { BRIDGE_CHAINS } from '../../utils/bridgeChains'
import { DEFAULT_NETWORK, NETWORKS } from '../../utils/constants'
import { sounds } from '../../lib/sounds'
import type { NetworkTier } from '../../utils/bridgeChains'

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const FAUCET_ABI = [
  {
    name: 'claim',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [{ name: 'token', type: 'address' }],
    outputs: [],
  },
  {
    name: 'claimableAt',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'user', type: 'address' },
      { name: 'token', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const

interface ChainConfig {
  key: string
  name: string
  chainId: number | string
  type: 'evm' | 'cosmos'
  faucetAddress: string
  explorerTxUrl: string
}

interface TokenConfig {
  symbol: string
  label: string
  addresses: Record<string, string>
}

const EVM_CHAINS: ChainConfig[] = [
  {
    key: 'bsc',
    name: 'BNB Chain',
    chainId: 56,
    type: 'evm',
    faucetAddress: import.meta.env.VITE_BSC_FAUCET_ADDRESS || '',
    explorerTxUrl: 'https://bscscan.com/tx/',
  },
  {
    key: 'opbnb',
    name: 'opBNB',
    chainId: 204,
    type: 'evm',
    faucetAddress: import.meta.env.VITE_OPBNB_FAUCET_ADDRESS || '',
    explorerTxUrl: 'https://opbnb.bscscan.com/tx/',
  },
]

const TERRA_CHAIN: ChainConfig = {
  key: 'terra',
  name: 'Terra Classic',
  chainId: 'columbus-5',
  type: 'cosmos',
  faucetAddress: import.meta.env.VITE_TERRA_FAUCET_ADDRESS || '',
  explorerTxUrl: 'https://finder.terraclassic.community/mainnet/tx/',
}

const ALL_CHAINS: ChainConfig[] = [...EVM_CHAINS, ...(TERRA_CHAIN.faucetAddress ? [TERRA_CHAIN] : [])]

const TOKENS: TokenConfig[] = [
  {
    symbol: 'testa',
    label: 'Test A (testa-cb)',
    addresses: {
      bsc: '0xD68393098E9252A2c377F3474C38B249D7bd5D92',
      opbnb: '0xB3a6385f4B4879cb5CB3188A574cCA0E82614bE1',
      terra: 'terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh',
    },
  },
  {
    symbol: 'testb',
    label: 'Test B (testb-cb)',
    addresses: {
      bsc: '0x65FFbA340768BadEc8002C76a542931757372d58',
      opbnb: '0x741dCAcE81e0F161f6A8f424B66d4b2bee3F29F6',
      terra: 'terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3',
    },
  },
  {
    symbol: 'tdec',
    label: 'Test Dec (tdec-cb)',
    addresses: {
      bsc: '0xC62351E2445AB732289e07Be795149Bc774bB043',
      opbnb: '0xcd733526bf0b48ad7fad597fc356ff8dc3aa103d',
      terra: 'terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv',
    },
  },
]

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatCountdown(seconds: number): string {
  if (seconds <= 0) return 'now'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

// ---------------------------------------------------------------------------
// Per-claim button component (EVM)
// ---------------------------------------------------------------------------

function EvmClaimButton({
  chain,
  tokenAddress,
}: {
  chain: ChainConfig
  tokenAddress: string
}) {
  const { address: userAddress, isConnected } = useAccount()
  const { switchChainAsync } = useSwitchChain()
  const { writeContractAsync } = useWriteContract()
  const [txHash, setTxHash] = useState<`0x${string}` | undefined>()
  const [status, setStatus] = useState<'idle' | 'switching' | 'claiming' | 'waiting' | 'success' | 'error'>('idle')
  const [error, setError] = useState<string | null>(null)
  const [countdown, setCountdown] = useState<number | null>(null)

  const faucetAddr = chain.faucetAddress as `0x${string}`
  const tokenAddr = tokenAddress as `0x${string}`

  const bridgeChainConfig = useMemo(() => {
    const tier = DEFAULT_NETWORK as NetworkTier
    const config = BRIDGE_CHAINS[tier]?.[chain.key]
    return config?.type === 'evm' ? config : null
  }, [chain.key])

  const { data: claimableAt, refetch: refetchClaimable } = useQuery({
    queryKey: ['faucetClaimableAt', chain.key, userAddress, tokenAddr],
    queryFn: async () => {
      if (!userAddress || !faucetAddr || !bridgeChainConfig) return undefined
      const client = getEvmClient(bridgeChainConfig as Parameters<typeof getEvmClient>[0])
      return client.readContract({
        address: faucetAddr,
        abi: FAUCET_ABI,
        functionName: 'claimableAt',
        args: [userAddress, tokenAddr],
      })
    },
    enabled: !!userAddress && !!faucetAddr && !!bridgeChainConfig,
    refetchInterval: 30_000,
    staleTime: 15_000,
  })

  const { isSuccess: txConfirmed, isError: txFailed } = useWaitForTransactionReceipt({
    hash: txHash,
    query: { enabled: !!txHash && status === 'waiting' },
  })

  useEffect(() => {
    if (txConfirmed && status === 'waiting') {
      setStatus('success')
      sounds.playSuccess()
      refetchClaimable()
    }
    if (txFailed && status === 'waiting') {
      setStatus('error')
      setError('Transaction failed on-chain')
    }
  }, [txConfirmed, txFailed, status, refetchClaimable])

  // Countdown timer
  useEffect(() => {
    if (claimableAt === undefined) return
    const target = Number(claimableAt)
    if (target === 0) {
      setCountdown(null)
      return
    }
    const update = () => {
      const now = Math.floor(Date.now() / 1000)
      const remaining = target - now
      setCountdown(remaining > 0 ? remaining : null)
    }
    update()
    const interval = setInterval(update, 1000)
    return () => clearInterval(interval)
  }, [claimableAt])

  const handleClaim = useCallback(async () => {
    if (!isConnected || !userAddress) return
    setError(null)
    setTxHash(undefined)

    try {
      setStatus('switching')
      try {
        await switchChainAsync({ chainId: chain.chainId as Parameters<typeof switchChainAsync>[0]['chainId'] })
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Failed to switch chain'
        if (msg.toLowerCase().includes('rejected') || msg.toLowerCase().includes('denied')) {
          setStatus('idle')
          return
        }
        throw e
      }

      setStatus('claiming')
      const hash = await writeContractAsync({
        address: faucetAddr,
        abi: FAUCET_ABI,
        functionName: 'claim',
        args: [tokenAddr],
      })
      setTxHash(hash)
      setStatus('waiting')
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Claim failed'
      if (msg.toLowerCase().includes('rejected') || msg.toLowerCase().includes('denied')) {
        setStatus('idle')
        return
      }
      setStatus('error')
      setError(msg.length > 120 ? msg.slice(0, 120) + '...' : msg)
    }
  }, [isConnected, userAddress, chain.chainId, faucetAddr, tokenAddr, switchChainAsync, writeContractAsync])

  if (!chain.faucetAddress) {
    return <span className="text-xs text-gray-500">Not deployed</span>
  }

  if (!isConnected) {
    return <span className="text-xs text-gray-500">Connect EVM wallet</span>
  }

  const isOnCooldown = countdown !== null && countdown > 0
  const isBusy = status === 'switching' || status === 'claiming' || status === 'waiting'

  return (
    <div className="flex flex-col gap-1">
      <button
        type="button"
        disabled={isBusy || isOnCooldown}
        onClick={() => {
          sounds.playButtonPress()
          handleClaim()
        }}
        className={`px-3 py-1.5 text-xs font-medium border transition-colors ${
          isBusy
            ? 'border-yellow-500/40 bg-yellow-900/20 text-yellow-300 cursor-wait'
            : isOnCooldown
              ? 'border-white/10 bg-black/20 text-gray-500 cursor-not-allowed'
              : status === 'success'
                ? 'border-green-500/40 bg-green-900/20 text-green-300'
                : 'border-white/20 bg-[#161616] text-slate-300 hover:border-[#b8ff3d]/60 hover:text-white'
        }`}
      >
        {isBusy && <Spinner className="inline mr-1 h-3 w-3" />}
        {status === 'switching' && 'Switching...'}
        {status === 'claiming' && 'Sign tx...'}
        {status === 'waiting' && 'Confirming...'}
        {status === 'success' && 'Claimed!'}
        {(status === 'idle' || status === 'error') &&
          (isOnCooldown ? formatCountdown(countdown) : `${chain.name}`)}
      </button>
      {status === 'success' && txHash && (
        <a
          href={`${chain.explorerTxUrl}${txHash}`}
          target="_blank"
          rel="noopener noreferrer"
          className="text-[10px] text-cyan-300 hover:text-cyan-200 truncate max-w-[140px]"
        >
          View tx →
        </a>
      )}
      {status === 'error' && error && (
        <p className="text-[10px] text-red-400 max-w-[160px] break-words">{error}</p>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Per-claim button component (Terra)
// ---------------------------------------------------------------------------

function TerraClaimButton({
  chain,
  tokenAddress,
}: {
  chain: ChainConfig
  tokenAddress: string
}) {
  const { address: terraAddress, connected: terraConnected } = useWalletStore()
  const [status, setStatus] = useState<'idle' | 'claiming' | 'success' | 'error'>('idle')
  const [error, setError] = useState<string | null>(null)
  const [txHash, setTxHash] = useState<string | null>(null)
  const [countdown, setCountdown] = useState<number | null>(null)

  // Query claimableAt from Terra faucet contract
  useEffect(() => {
    if (!terraConnected || !terraAddress || !chain.faucetAddress || !tokenAddress) return

    const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
    const lcdUrls =
      networkConfig.lcdFallbacks && networkConfig.lcdFallbacks.length > 0
        ? [...networkConfig.lcdFallbacks]
        : [networkConfig.lcd]

    queryContract<{ claimable_at: number }>(lcdUrls, chain.faucetAddress, {
      claimable_at: { user: terraAddress, token: tokenAddress },
    })
      .then((res) => {
        if (res.claimable_at === 0) {
          setCountdown(null)
        } else {
          const remaining = res.claimable_at - Math.floor(Date.now() / 1000)
          setCountdown(remaining > 0 ? remaining : null)
        }
      })
      .catch(() => {
        // Faucet may not be deployed yet
      })
  }, [terraConnected, terraAddress, chain.faucetAddress, tokenAddress, status])

  // Tick countdown
  useEffect(() => {
    if (countdown === null || countdown <= 0) return
    const interval = setInterval(() => {
      setCountdown((prev) => (prev !== null && prev > 1 ? prev - 1 : null))
    }, 1000)
    return () => clearInterval(interval)
  }, [countdown])

  const handleClaim = useCallback(async () => {
    if (!terraConnected || !terraAddress || !chain.faucetAddress) return
    setError(null)
    setTxHash(null)
    setStatus('claiming')

    try {
      const result = await executeContractWithCoins(chain.faucetAddress, {
        claim: { token: tokenAddress },
      })
      setTxHash(result.txHash)
      setStatus('success')
      sounds.playSuccess()
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Claim failed'
      if (msg.toLowerCase().includes('rejected') || msg.toLowerCase().includes('denied')) {
        setStatus('idle')
        return
      }
      setStatus('error')
      setError(msg.length > 120 ? msg.slice(0, 120) + '...' : msg)
    }
  }, [terraConnected, terraAddress, chain.faucetAddress, tokenAddress])

  if (!chain.faucetAddress) {
    return <span className="text-xs text-gray-500">Not deployed</span>
  }

  if (!terraConnected) {
    return <span className="text-xs text-gray-500">Connect Terra wallet</span>
  }

  const isOnCooldown = countdown !== null && countdown > 0
  const isBusy = status === 'claiming'

  return (
    <div className="flex flex-col gap-1">
      <button
        type="button"
        disabled={isBusy || isOnCooldown}
        onClick={() => {
          sounds.playButtonPress()
          handleClaim()
        }}
        className={`px-3 py-1.5 text-xs font-medium border transition-colors ${
          isBusy
            ? 'border-yellow-500/40 bg-yellow-900/20 text-yellow-300 cursor-wait'
            : isOnCooldown
              ? 'border-white/10 bg-black/20 text-gray-500 cursor-not-allowed'
              : status === 'success'
                ? 'border-green-500/40 bg-green-900/20 text-green-300'
                : 'border-white/20 bg-[#161616] text-slate-300 hover:border-[#b8ff3d]/60 hover:text-white'
        }`}
      >
        {isBusy && <Spinner className="inline mr-1 h-3 w-3" />}
        {status === 'claiming' && 'Claiming...'}
        {status === 'success' && 'Claimed!'}
        {(status === 'idle' || status === 'error') &&
          (isOnCooldown ? formatCountdown(countdown) : `${chain.name}`)}
      </button>
      {status === 'success' && txHash && (
        <a
          href={`${chain.explorerTxUrl}${txHash}`}
          target="_blank"
          rel="noopener noreferrer"
          className="text-[10px] text-cyan-300 hover:text-cyan-200 truncate max-w-[140px]"
        >
          View tx →
        </a>
      )}
      {status === 'error' && error && (
        <p className="text-[10px] text-red-400 max-w-[160px] break-words">{error}</p>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main FaucetPanel
// ---------------------------------------------------------------------------

export function FaucetPanel() {
  const hasAnyFaucet = ALL_CHAINS.some((c) => !!c.faucetAddress)

  if (!hasAnyFaucet) {
    return (
      <div className="border-2 border-yellow-700/50 bg-yellow-900/15 p-4">
        <p className="text-sm text-yellow-300">
          No faucet contracts configured. Set <code className="text-xs">VITE_BSC_FAUCET_ADDRESS</code>,{' '}
          <code className="text-xs">VITE_OPBNB_FAUCET_ADDRESS</code>, or{' '}
          <code className="text-xs">VITE_TERRA_FAUCET_ADDRESS</code> in your environment.
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-medium uppercase tracking-wider text-gray-300">
          Test Token Faucet
        </h3>
        <p className="mt-1 text-xs text-gray-400">
          Claim 10 test tokens per wallet per token per chain, once every 24 hours.
        </p>
      </div>

      <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-3">
        {TOKENS.map((token) => (
          <Card key={token.symbol} className="p-4">
            <h4 className="mb-3 font-medium text-white">{token.label}</h4>
            <div className="space-y-2">
              {ALL_CHAINS.map((chain) => {
                const addr = token.addresses[chain.key]
                if (!addr) return null
                return (
                  <div key={chain.key} className="flex items-center justify-between gap-2">
                    <span className="text-xs text-gray-400 w-20 shrink-0">{chain.name}</span>
                    {chain.type === 'evm' ? (
                      <EvmClaimButton chain={chain} tokenAddress={addr} />
                    ) : (
                      <TerraClaimButton chain={chain} tokenAddress={addr} />
                    )}
                  </div>
                )
              })}
            </div>
          </Card>
        ))}
      </div>
    </div>
  )
}
