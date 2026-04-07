/**
 * Transfer UI token row (bridge token picker, amount input, etc.).
 * Shared by services (e.g. buildTransferTokens) and components (TokenSelect).
 */
export interface TokenOption {
  id: string
  symbol: string
  tokenId: string
  /** EVM token address when source is EVM — used for onchain symbol lookup */
  evmTokenAddress?: string
}
