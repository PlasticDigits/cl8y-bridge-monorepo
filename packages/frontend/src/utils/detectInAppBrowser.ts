export interface InAppBrowserInfo {
  isInAppBrowser: boolean
  browserName: string | null
}

const WALLET_PATTERNS: [RegExp, string][] = [
  [/Keplr/i, 'Keplr'],
  [/TrustWallet|Trust\//i, 'Trust Wallet'],
  [/MetaMaskMobile/i, 'MetaMask'],
  [/CoinbaseWallet|CoinbaseBrowser/i, 'Coinbase Wallet'],
  [/ImToken/i, 'imToken'],
  [/TokenPocket/i, 'TokenPocket'],
  [/OKApp/i, 'OKX Wallet'],
]

const SOCIAL_PATTERNS: [RegExp, string][] = [
  [/FBAN|FBAV/i, 'Facebook'],
  [/Instagram/i, 'Instagram'],
  [/Line\//i, 'LINE'],
  [/Twitter|X\//i, 'X (Twitter)'],
  [/Telegram/i, 'Telegram'],
  [/Discord/i, 'Discord'],
]

/**
 * Detects whether the current browser is an in-app browser (wallet WebView,
 * social app WebView, or generic Android/iOS WebView). In-app browsers
 * typically cannot handle custom URL schemes like `trust://` or `keplrwallet://`,
 * which breaks WalletConnect deep links.
 */
export function detectInAppBrowser(): InAppBrowserInfo {
  if (typeof navigator === 'undefined') return { isInAppBrowser: false, browserName: null }

  const ua = navigator.userAgent || ''

  for (const [pattern, name] of WALLET_PATTERNS) {
    if (pattern.test(ua)) return { isInAppBrowser: true, browserName: name }
  }

  for (const [pattern, name] of SOCIAL_PATTERNS) {
    if (pattern.test(ua)) return { isInAppBrowser: true, browserName: name }
  }

  // Android WebView: contains "; wv)" in the UA
  if (/; wv\)/.test(ua)) return { isInAppBrowser: true, browserName: 'WebView' }

  // iOS WKWebView heuristic: has "AppleWebKit" but not "Safari"
  if (/iPhone|iPad|iPod/.test(ua) && /AppleWebKit/.test(ua) && !/Safari/.test(ua)) {
    return { isInAppBrowser: true, browserName: 'In-App Browser' }
  }

  return { isInAppBrowser: false, browserName: null }
}
