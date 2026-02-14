/** UI sounds - paths relative to public */
const SOUNDS = {
  buttonPress: '/sounds/button-press.wav',
  hover: '/sounds/hover.wav',
  success: '/sounds/success.wav',
  error: '/sounds/error.wav',
} as const

let buttonPressAudio: HTMLAudioElement | null = null
let hoverAudio: HTMLAudioElement | null = null
let successAudio: HTMLAudioElement | null = null
let errorAudio: HTMLAudioElement | null = null

function getAudio(key: keyof typeof SOUNDS): HTMLAudioElement | null {
  switch (key) {
    case 'buttonPress':
      if (!buttonPressAudio) buttonPressAudio = new Audio(SOUNDS.buttonPress)
      return buttonPressAudio
    case 'hover':
      if (!hoverAudio) hoverAudio = new Audio(SOUNDS.hover)
      return hoverAudio
    case 'success':
      if (!successAudio) successAudio = new Audio(SOUNDS.success)
      return successAudio
    case 'error':
      if (!errorAudio) errorAudio = new Audio(SOUNDS.error)
      return errorAudio
  }
}

function play(key: keyof typeof SOUNDS): void {
  const audio = getAudio(key)
  if (!audio) return
  try {
    audio.currentTime = 0
    audio.volume = key === 'buttonPress' ? 0.2 : 0.4
    audio.play().catch(() => {})
  } catch {
    // ignore
  }
}

export const sounds = {
  playButtonPress: () => play('buttonPress'),
  playHover: () => play('hover'),
  playSuccess: () => play('success'),
  playError: () => play('error'),
}
