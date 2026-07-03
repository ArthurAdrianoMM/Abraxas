/** The Abraxas seal — concentric circles crossed by the vertical axis. */
export function AbraxasGlyph({ size = 30 }: { size?: number }) {
  return (
    <svg
      className="glyph"
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
      aria-hidden="true"
    >
      <circle cx="16" cy="16" r="13" stroke="#b89968" strokeWidth="0.9" fill="none" />
      <circle cx="16" cy="16" r="9.5" stroke="#7d2233" strokeWidth="0.7" fill="none" />
      <line x1="16" y1="1.5" x2="16" y2="30.5" stroke="#b89968" strokeWidth="0.9" />
      <line x1="11" y1="16" x2="21" y2="16" stroke="#7d2233" strokeWidth="0.7" />
      <circle cx="16" cy="11" r="1.4" fill="#b89968" />
    </svg>
  );
}
