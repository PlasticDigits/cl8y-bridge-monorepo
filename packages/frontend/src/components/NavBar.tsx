import { NavLink } from 'react-router-dom'
import { ConnectWallet } from './ConnectWallet'
import { WalletButton } from './WalletButton'

export function NavBar() {
  return (
    <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
      <div className="flex items-center justify-between min-h-16 py-3 gap-3">
        <div className="flex items-center gap-3 md:gap-6 min-w-0">
          <NavLink to="/" className="flex items-center gap-3 group">
            <div className="w-8 h-8 border-2 border-black bg-[#b8ff3d] shadow-[3px_3px_0_#000] group-hover:translate-x-[1px] group-hover:translate-y-[1px] transition-transform" />
            <span className="hidden sm:inline text-xl font-semibold text-slate-100 tracking-[0.08em] uppercase">CL8Y Bridge</span>
          </NavLink>
          <nav className="flex gap-1 border-2 border-white/30 bg-black/80 p-1 overflow-x-auto">
            <NavLink
              to="/"
              end
              className={({ isActive }) =>
                `px-2.5 md:px-3.5 py-2 text-xs md:text-sm font-medium whitespace-nowrap uppercase tracking-wide border ${
                  isActive
                    ? 'bg-[#b8ff3d] text-black border-black shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              Bridge
            </NavLink>
            <NavLink
              to="/history"
              className={({ isActive }) =>
                `px-2.5 md:px-3.5 py-2 text-xs md:text-sm font-medium whitespace-nowrap uppercase tracking-wide border ${
                  isActive
                    ? 'bg-[#b8ff3d] text-black border-black shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              History
            </NavLink>
            <NavLink
              to="/verify"
              className={({ isActive }) =>
                `px-2.5 md:px-3.5 py-2 text-xs md:text-sm font-medium whitespace-nowrap uppercase tracking-wide border ${
                  isActive
                    ? 'bg-[#b8ff3d] text-black border-black shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              Verify
            </NavLink>
            <NavLink
              to="/settings"
              className={({ isActive }) =>
                `px-2.5 md:px-3.5 py-2 text-xs md:text-sm font-medium whitespace-nowrap uppercase tracking-wide border ${
                  isActive
                    ? 'bg-[#b8ff3d] text-black border-black shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              Settings
            </NavLink>
          </nav>
        </div>
        <div className="flex items-center gap-2 sm:gap-3">
          <WalletButton />
          <ConnectWallet />
        </div>
      </div>
    </div>
  )
}
