export interface SwapDirectionButtonProps {
  onClick: () => void
  disabled?: boolean
}

export function SwapDirectionButton({ onClick, disabled }: SwapDirectionButtonProps) {
  return (
    <div className="flex justify-center -mb-3">
      <button
        type="button"
        onClick={onClick}
        disabled={disabled}
        className="border-2 border-white/20 bg-[#161616] p-2 hover:border-cyan-300 hover:text-cyan-200 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <svg
          className="h-4 w-4 text-gray-300"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"
          />
        </svg>
      </button>
    </div>
  )
}
