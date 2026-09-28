type BrandMarkProps = { size?: 'sm' | 'md' }

export function BrandMark({ size = 'md' }: BrandMarkProps) {
  const bars = [10, 18, 27, 16, 23, 12]
  return (
    <span className={`brand-mark brand-mark--${size}`} aria-hidden="true">
      {bars.map((height, index) => <i key={index} style={{ height }} />)}
    </span>
  )
}
