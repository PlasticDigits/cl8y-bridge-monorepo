import { NavLink } from 'react-router-dom'
import { ConnectWallet } from './ConnectWallet'
import { WalletButton } from './WalletButton'

export function NavBar() {
  return (
    <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
      <div className="flex flex-col xl:flex-row xl:items-center xl:justify-between min-h-14 py-2 gap-2">
        <div className="flex items-center shrink-0">
          <NavLink to="/" className="flex items-center group -my-1">
            <img
              src="/logo-header.png"
              alt="CL8Y Bridge"
              className="h-10 xl:h-12 w-auto max-w-[150px] object-contain object-left group-hover:translate-x-[1px] group-hover:translate-y-[1px] transition-transform"
            />
          </NavLink>
        </div>

        <div className="hidden min-[480px]:flex items-center gap-2 xl:hidden shrink-0">
          <WalletButton />
          <ConnectWallet />
        </div>

        <div className="grid grid-cols-2 gap-2 w-full min-[480px]:hidden [&>button]:w-full [&>button]:justify-center">
          <WalletButton />
          <ConnectWallet />
        </div>

        <nav className="flex gap-1 border-2 border-white/30 bg-black/80 p-1 min-w-0 w-full xl:w-auto xl:flex-1">
          <NavLink
              to="/"
              end
              className={({ isActive }) =>
                `max-[479px]:flex-1 text-center px-1.5 min-[480px]:px-2.5 xl:px-3.5 py-2 text-[11px] min-[480px]:text-xs xl:text-sm font-medium whitespace-nowrap uppercase tracking-[0.04em] min-[480px]:tracking-wide border ${
                  isActive
                    ? 'bg-[#202614] text-[#d5ff7f] border-[#b8ff3d]/60 shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              Bridge
            </NavLink>
            <NavLink
              to="/history"
              className={({ isActive }) =>
                `max-[479px]:flex-1 text-center px-1.5 min-[480px]:px-2.5 xl:px-3.5 py-2 text-[11px] min-[480px]:text-xs xl:text-sm font-medium whitespace-nowrap uppercase tracking-[0.04em] min-[480px]:tracking-wide border ${
                  isActive
                    ? 'bg-[#202614] text-[#d5ff7f] border-[#b8ff3d]/60 shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              History
            </NavLink>
            <NavLink
              to="/verify"
              className={({ isActive }) =>
                `max-[479px]:flex-1 text-center px-1.5 min-[480px]:px-2.5 xl:px-3.5 py-2 text-[11px] min-[480px]:text-xs xl:text-sm font-medium whitespace-nowrap uppercase tracking-[0.04em] min-[480px]:tracking-wide border ${
                  isActive
                    ? 'bg-[#202614] text-[#d5ff7f] border-[#b8ff3d]/60 shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              Verify
            </NavLink>
            <NavLink
              to="/settings"
              className={({ isActive }) =>
                `max-[479px]:flex-1 text-center px-1.5 min-[480px]:px-2.5 xl:px-3.5 py-2 text-[11px] min-[480px]:text-xs xl:text-sm font-medium whitespace-nowrap uppercase tracking-[0.04em] min-[480px]:tracking-wide border ${
                  isActive
                    ? 'bg-[#202614] text-[#d5ff7f] border-[#b8ff3d]/60 shadow-[2px_2px_0_#000]'
                    : 'text-slate-200 border-transparent hover:border-white/40 hover:bg-zinc-800'
                }`
              }
            >
              Settings
          </NavLink>
        </nav>
        <div className="hidden xl:flex items-center gap-2 shrink-0">
          <WalletButton />
          <ConnectWallet />
        </div>
      </div>
    </div>
  )
}
