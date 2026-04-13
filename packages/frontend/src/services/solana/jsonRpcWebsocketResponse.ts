/**
 * Helpers documenting JSON-RPC response handling for `rpc-websockets` (used by
 * `@solana/web3.js` for `Connection` WebSocket calls such as `signatureSubscribe`
 * during `confirmTransaction`).
 *
 * Upstream `rpc-websockets` v9.3.x used `("error" in msg) === ("result" in msg)` to
 * detect malformed responses. That is true when **both** keys exist (including
 * `error: null` with `result`), which some Solana RPCs send — triggering a false
 * "Server response malformed" and breaking confirmations (GitLab #106).
 *
 * @see ../../patches/rpc-websockets+9.3.3.patch
 */

/** Behavior of unpatched rpc-websockets (buggy for `error: null` + `result`). */
export function rpcWebsocketMalformedCheckUnpatched(
  message: Record<string, unknown>,
): boolean {
  return ("error" in message) === ("result" in message);
}

/** Behavior after patch-package fix: only reject when neither key is present. */
export function rpcWebsocketMalformedCheckPatched(
  message: Record<string, unknown>,
): boolean {
  return (
    !Object.prototype.hasOwnProperty.call(message, "error") &&
    !Object.prototype.hasOwnProperty.call(message, "result")
  );
}
