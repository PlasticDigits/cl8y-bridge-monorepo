/**
 * Token deployment orchestrator for E2E test setup.
 * Deploys 3 test tokens (TokenA, TokenB, TokenC) across all 3 chains.
 */

import { deployThreeTokens } from './deploy-evm'
import { deployThreeCw20Tokens } from './deploy-terra'

export interface TokenAddresses {
  anvil: { tokenA: string; tokenB: string; tokenC: string }
  anvil1: { tokenA: string; tokenB: string; tokenC: string }
  terra: { tokenA: string; tokenB: string; tokenC: string }
}

/**
 * Deploy 3 tokens to all 3 chains.
 */
export function deployAllTokens(terraBridgeAddress: string): TokenAddresses {
  console.log('[deploy-tokens] Deploying tokens across all chains...')

  // Deploy ERC20 tokens to both Anvil chains
  const anvilTokens = deployThreeTokens('http://localhost:8545')
  const anvil1Tokens = deployThreeTokens('http://localhost:8546')

  // Deploy CW20 tokens to LocalTerra
  const terraTokens = deployThreeCw20Tokens(terraBridgeAddress)

  const result: TokenAddresses = {
    anvil: {
      tokenA: anvilTokens.tokenAAddress,
      tokenB: anvilTokens.tokenBAddress,
      tokenC: anvilTokens.tokenCAddress,
    },
    anvil1: {
      tokenA: anvil1Tokens.tokenAAddress,
      tokenB: anvil1Tokens.tokenBAddress,
      tokenC: anvil1Tokens.tokenCAddress,
    },
    terra: {
      tokenA: terraTokens.tokenA.tokenAddress,
      tokenB: terraTokens.tokenB.tokenAddress,
      tokenC: terraTokens.tokenC.tokenAddress,
    },
  }

  console.log('[deploy-tokens] All tokens deployed:', JSON.stringify(result, null, 2))
  return result
}
