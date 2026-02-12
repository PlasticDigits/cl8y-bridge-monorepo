import { WalletName } from '@goblinhunt/cosmes/wallet'
import {
  StationIcon,
  KeplrIcon,
  LeapIcon,
  CosmostationIcon,
  LuncDashIcon,
  GalaxyIcon,
} from './WalletIcons'

export interface TerraWalletOptionProps {
  name: string
  description: string
  available: boolean
  loading?: boolean
  onClick: () => void
  disabled?: boolean
  icon?: React.ReactNode
}

const ICON_MAP: Partial<Record<WalletName, React.ReactNode>> = {
  [WalletName.STATION]: <StationIcon />,
  [WalletName.KEPLR]: <KeplrIcon />,
  [WalletName.LEAP]: <LeapIcon />,
  [WalletName.COSMOSTATION]: <CosmostationIcon />,
  [WalletName.LUNCDASH]: <LuncDashIcon />,
  [WalletName.GALAXYSTATION]: <GalaxyIcon />,
}

export function getTerraWalletIcon(walletName: WalletName): React.ReactNode {
  return ICON_MAP[walletName] ?? (
    <div className="w-10 h-10 rounded-xl bg-amber-500/20 flex items-center justify-center">
      <span className="text-amber-400">🌙</span>
    </div>
  )
}

export function TerraWalletOption({
  name,
  description,
  available,
  loading,
  onClick,
  disabled,
  icon,
}: TerraWalletOptionProps) {
  const displayIcon = icon ?? (
    <div className="w-10 h-10 rounded-xl bg-amber-500/20 flex items-center justify-center">
      <span className="text-amber-400">🌙</span>
    </div>
  )
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled || !available}
      className={`
        w-full flex items-center gap-4 p-4 rounded-xl border transition-all duration-200
        ${available && !disabled
          ? 'border-white/5 hover:border-amber-500/40 hover:bg-amber-500/5 hover:shadow-lg hover:shadow-amber-500/5 cursor-pointer group'
          : 'border-white/5 opacity-40 cursor-not-allowed'
        }
      `}
    >
      {displayIcon}
      <div className="flex-1 text-left">
        <p className="font-medium text-white group-hover:text-amber-50 transition-colors">{name}</p>
        <p className="text-xs text-gray-500">{description}</p>
      </div>
      {loading ? (
        <svg className="w-5 h-5 text-amber-400 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
        </svg>
      ) : available && !disabled ? (
        <svg className="w-5 h-5 text-gray-600 group-hover:text-amber-500/70 transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
      ) : null}
    </button>
  )
}
