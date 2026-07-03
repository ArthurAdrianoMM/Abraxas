import styles from "./SealDial.module.css";

/** The hermetic dial from the Download design: outer/inner rings, 12 tick
 *  marks (cardinals emphasized) and a progress track + fill. `progress` is
 *  0–100; the fill transitions smoothly. Children render at the center. */
export function SealDial({
  progress,
  state,
  children,
  showCheck = false,
  armed = false,
}: {
  progress: number;
  /** mirrors the design's data-state styling hooks. */
  state: "confirm" | "connecting" | "downloading" | "paused" | "verifying" | "complete" | "failed";
  children?: React.ReactNode;
  /** brass check path drawn over the dial (complete state). */
  showCheck?: boolean;
  /** confirm-step brass dot at 12 o'clock. */
  armed?: boolean;
}) {
  const offset = Math.max(0, Math.min(100, 100 - progress));
  return (
    <div className={styles.seal} data-state={state}>
      <svg viewBox="0 0 240 240" aria-hidden="true">
        <g className={styles.ticks}>
          <line x1="120" y1="14" x2="120" y2="22" className={styles.cardinal} />
          <line x1="120" y1="218" x2="120" y2="226" className={styles.cardinal} />
          <line x1="14" y1="120" x2="22" y2="120" className={styles.cardinal} />
          <line x1="218" y1="120" x2="226" y2="120" className={styles.cardinal} />
          <line x1="171" y1="22.4" x2="167.7" y2="29.7" />
          <line x1="217.6" y1="69" x2="210.3" y2="72.3" />
          <line x1="217.6" y1="171" x2="210.3" y2="167.7" />
          <line x1="171" y1="217.6" x2="167.7" y2="210.3" />
          <line x1="69" y1="217.6" x2="72.3" y2="210.3" />
          <line x1="22.4" y1="171" x2="29.7" y2="167.7" />
          <line x1="22.4" y1="69" x2="29.7" y2="72.3" />
          <line x1="69" y1="22.4" x2="72.3" y2="29.7" />
        </g>
        <circle cx="120" cy="120" r="108" className={styles.ringOuter} />
        <circle cx="120" cy="120" r="98" className={styles.ringInner} />
        <circle cx="120" cy="120" r="86" className={styles.dialTrack} />
        <circle
          cx="120"
          cy="120"
          r="86"
          className={styles.dialFill}
          pathLength={100}
          strokeDasharray="100"
          strokeDashoffset={offset.toFixed(2)}
        />
        {armed && <circle cx="120" cy="34" r="3.4" className={styles.armingDot} />}
        {showCheck && (
          <path
            d="M96 120 l16 16 l36 -36"
            stroke="#b89968"
            strokeWidth="2.2"
            fill="none"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        )}
      </svg>
      <div className={styles.verifyGlyph} />
      <div className={styles.center}>{children}</div>
    </div>
  );
}
