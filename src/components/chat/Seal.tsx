/** The small abraxas seal that marks assistant turns (from the design mocks). */
export function Seal({ size = 11 }: { size?: number }) {
  return (
    <span className="seal" aria-hidden="true">
      <svg width={size} height={size} viewBox="0 0 32 32" fill="none">
        <circle cx="16" cy="16" r="13" stroke="#7d2233" strokeWidth="1.4" fill="none" />
        <line x1="16" y1="3" x2="16" y2="29" stroke="#7d2233" strokeWidth="1.3" />
        <circle cx="16" cy="11" r="1.7" fill="#b89968" />
      </svg>
    </span>
  );
}
