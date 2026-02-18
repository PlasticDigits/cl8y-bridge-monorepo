/**
 * Token deployment orchestrator for E2E test setup.
 * Deploys 3 test tokens (TokenA, TokenB, TokenC) + LUNC (uluna representation)
 * + KDEC (decimal normalization test token) across all 3 chains.
 */

import { deployThreeTokens, deployLuncToken, deployKdecToken } from './deploy-evm'
import { deployThreeCw20Tokens, deployCw20KdecToken } from './deploy-terra'

export interface TokenAddresses {
  anvil: { tokenA: string; tokenB: string; tokenC: string; lunc: string; kdec: string }
  anvil1: { tokenA: string; tokenB: string; tokenC: string; lunc: string; kdec: string }
  terra: { tokenA: string; tokenB: string; tokenC: string; kdec: string }
}

/** Per-chain decimals for KDEC token (used for cross-chain decimal normalization testing) */
export const KDEC_DECIMALS = {
  anvil: 18,
  anvil1: 12,
  terra: 6,
} as const

/**
 * Deploy 3 tokens + LUNC + KDEC to all 3 chains.
 * LUNC (tLUNC) is the EVM representation of Terra uluna - symbol shows as LUNC in UI.
 * KDEC has different decimals per chain (18/12/6) for decimal normalization testing.
 */
export function deployAllTokens(terraBridgeAddress: string): TokenAddresses {
  console.log('[deploy-tokens] Deploying tokens across all chains...')

  // Deploy ERC20 tokens to both Anvil chains
  const anvilTokens = deployThreeTokens('http://localhost:8545')
  const anvil1Tokens = deployThreeTokens('http://localhost:8546')

  // Deploy LUNC (uluna representation) on both EVM chains - symbol tLUNC
  const anvilLunc = deployLuncToken('http://localhost:8545')
  const anvil1Lunc = deployLuncToken('http://localhost:8546')

  // Deploy KDEC with different decimals per chain
  const anvilKdec = deployKdecToken('http://localhost:8545', KDEC_DECIMALS.anvil)
  const anvil1Kdec = deployKdecToken('http://localhost:8546', KDEC_DECIMALS.anvil1)

  // Deploy CW20 tokens to LocalTerra (TokenA/B/C + KDEC)
  const terraTokens = deployThreeCw20Tokens(terraBridgeAddress)
  const terraKdec = deployCw20KdecToken()

  const result: TokenAddresses = {
    anvil: {
      tokenA: anvilTokens.tokenAAddress,
      tokenB: anvilTokens.tokenBAddress,
      tokenC: anvilTokens.tokenCAddress,
      lunc: anvilLunc,
      kdec: anvilKdec,
    },
    anvil1: {
      tokenA: anvil1Tokens.tokenAAddress,
      tokenB: anvil1Tokens.tokenBAddress,
      tokenC: anvil1Tokens.tokenCAddress,
      lunc: anvil1Lunc,
      kdec: anvil1Kdec,
    },
    terra: {
      tokenA: terraTokens.tokenA.tokenAddress,
      tokenB: terraTokens.tokenB.tokenAddress,
      tokenC: terraTokens.tokenC.tokenAddress,
      kdec: terraKdec,
    },
  }

  console.log('[deploy-tokens] All tokens deployed:', JSON.stringify(result, null, 2))
  return result
}
