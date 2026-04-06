/**
 * Full-screen placeholder when VITE_UNDER_CONSTRUCTION=true.
 * No navigation or links; all routes render this view from main.tsx.
 */
export function UnderConstructionPage() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-[var(--bg-0)] p-6">
      <h1 className="sr-only">Under construction</h1>
      <img
        src="/under-construction.svg"
        alt=""
        className="max-h-[min(75vh,560px)] w-full max-w-2xl object-contain"
        width={480}
        height={360}
        decoding="async"
        fetchPriority="high"
      />
    </div>
  )
}
