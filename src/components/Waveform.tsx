type WaveformProps = { active?: number; dense?: boolean; onSeek?: (percent: number) => void }

const heights = [10, 18, 28, 42, 55, 34, 22, 14, 31, 49, 62, 44, 25, 20, 38, 53, 45, 29, 17, 26, 41, 34, 19, 13, 24, 38, 30, 16, 11, 22, 32, 18]

export function Waveform({ active = 52, dense = false, onSeek }: WaveformProps) {
  const seek = (clientX: number, element: HTMLDivElement) => {
    const bounds = element.getBoundingClientRect()
    onSeek?.(Math.max(0, Math.min(1, (clientX - bounds.left) / bounds.width)))
  }
  return (
    <div
      className={`waveform ${dense ? 'waveform--dense' : ''} ${onSeek ? 'is-seekable' : ''}`}
      aria-label="音频波形与播放进度"
      role={onSeek ? 'slider' : undefined}
      aria-valuemin={onSeek ? 0 : undefined}
      aria-valuemax={onSeek ? 100 : undefined}
      aria-valuenow={onSeek ? Math.round(active) : undefined}
      tabIndex={onSeek ? 0 : undefined}
      onClick={onSeek ? event => seek(event.clientX, event.currentTarget) : undefined}
      onKeyDown={onSeek ? event => {
        if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
          event.preventDefault()
          onSeek(Math.max(0, Math.min(1, (active + (event.key === 'ArrowRight' ? 5 : -5)) / 100)))
        }
      } : undefined}
    >
      {heights.map((height, index) => (
        <i key={index} className={index / heights.length * 100 < active ? 'is-played' : ''} style={{ height: `${height}%` }} />
      ))}
    </div>
  )
}
