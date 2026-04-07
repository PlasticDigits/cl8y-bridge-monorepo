/**
 * Token deployment orchestrator for E2E test setup.
 * Deploys 3 test tokens (TokenA, TokenB, TokenC) + LUNC (uluna representation)
 * + KDEC (decimal normalization test token) across all 3 chains.
 */

import { deployThreeTokens, deployLuncToken, deployKdecToken, deploySolToken, deployT2022TestToken } from './deploy-evm'
import { deployThreeCw20Tokens, deployCw20KdecToken, deployCw20SolToken, deployCw20T2022Token } from './deploy-terra'
import { deploySolanaMints, type SolanaTokenMints } from './deploy-solana'

export interface TokenAddresses {
  anvil: { tokenA: string; tokenB: string; tokenC: string; lunc: string; kdec: string; sol: string; t2022: string }
  anvil1: { tokenA: string; tokenB: string; tokenC: string; lunc: string; kdec: string; sol: string; t2022: string }
  terra: { tokenA: string; tokenB: string; tokenC: string; kdec: string; sol: string; t2022: string }
  solana: SolanaTokenMints
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
export async function deployAllTokens(terraBridgeAddress: string): Promise<TokenAddresses> {
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

  const anvilSol = deploySolToken('http://localhost:8545')
  const anvil1Sol = deploySolToken('http://localhost:8546')

  const anvilT2022 = deployT2022TestToken('http://localhost:8545')
  const anvil1T2022 = deployT2022TestToken('http://localhost:8546')

  // Deploy CW20 tokens to LocalTerra (TokenA/B/C + KDEC)
  const terraTokens = deployThreeCw20Tokens(terraBridgeAddress)
  const terraKdec = deployCw20KdecToken()
  const terraSol = deployCw20SolToken()
  const terraT2022 = deployCw20T2022Token()

  const solRpc = process.env.SOLANA_RPC_URL || 'http://127.0.0.1:8899'
  const home = process.env.HOME ?? process.env.USERPROFILE ?? ''
  const solKeypair = process.env.SOLANA_KEYPAIR || `${home}/.config/solana/id.json`
  const solana = await deploySolanaMints(solRpc, solKeypair)

  const result: TokenAddresses = {
    anvil: {
      tokenA: anvilTokens.tokenAAddress,
      tokenB: anvilTokens.tokenBAddress,
      tokenC: anvilTokens.tokenCAddress,
      lunc: anvilLunc,
      kdec: anvilKdec,
      sol: anvilSol,
      t2022: anvilT2022,
    },
    anvil1: {
      tokenA: anvil1Tokens.tokenAAddress,
      tokenB: anvil1Tokens.tokenBAddress,
      tokenC: anvil1Tokens.tokenCAddress,
      lunc: anvil1Lunc,
      kdec: anvil1Kdec,
      sol: anvil1Sol,
      t2022: anvil1T2022,
    },
    terra: {
      tokenA: terraTokens.tokenA.tokenAddress,
      tokenB: terraTokens.tokenB.tokenAddress,
      tokenC: terraTokens.tokenC.tokenAddress,
      kdec: terraKdec,
      sol: terraSol,
      t2022: terraT2022,
    },
    solana,
  }

  console.log('[deploy-tokens] All tokens deployed:', JSON.stringify(result, null, 2))
  return result
}
