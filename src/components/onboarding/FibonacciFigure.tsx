import styles from "./FibonacciFigure.module.css";

/* The plate that sits behind the onboarding screens: a golden-ratio
   construction — whirling squares and triangles, the Fibonacci tiling, and the
   logarithmic spiral itself — drawn as vectors rather than as the scanned GIF
   this replaces. That scan was opaque white paper with no alpha, so no filter
   or blend mode could do better than lighten a rectangle over the background;
   strokes have no paper to hide. Every figure here is generated from φ, so the
   geometry is exact rather than traced, and the strokes take their colour from
   the design tokens instead of being approximated with a sepia filter.

   Coordinates live in a 1000×1000 box centred on (500, 500); nothing reaches
   past r = 468 except the construction chords, which are meant to run off the
   edge and be clipped by the caller. */

const SPIRAL =
  "M497 498.9C497.1 498.6 497.2 497.7 497.6 497.1C498 496.6 498.5 496 499.2 495.7C499.9 495.3 500.9 495.1 501.8 495.1C502.7 495.2 503.8 495.5 504.7 496.1C505.6 496.7 506.5 497.7 507.1 498.8C507.6 499.9 508 501.5 507.9 502.9C507.7 504.4 507.3 506.2 506.3 507.6C505.3 509 503.7 510.6 501.9 511.4C500 512.3 497.6 512.9 495.2 512.7C492.8 512.5 489.9 511.7 487.6 510.1C485.3 508.5 482.8 506 481.5 503C480.1 500 479 496 479.4 492.1C479.8 488.3 481.1 483.6 483.7 479.9C486.4 476.2 490.4 472.2 495.3 470C500.1 467.8 506.7 466.1 512.9 466.7C519.1 467.3 526.7 469.4 532.7 473.7C538.7 478.1 545.2 484.7 548.7 492.6C552.2 500.5 554.9 511 553.9 521.1C552.8 531.2 549.3 543.5 542.3 553.1C535.3 562.8 524.5 573.3 511.7 578.9C498.9 584.6 481.8 588.9 465.5 587.1C449.1 585.3 429.2 579.7 413.6 568.2C398.1 556.8 381.1 539.3 372.1 518.5C363 497.7 356.1 470 359.1 443.5C362.1 417 371.4 384.8 390 359.6C408.7 334.5 437.1 307.2 470.9 292.6C504.6 278 549.6 267 592.5 272.1C635.4 277.1 687.5 292.4 728.1 322.8C768.8 353.1 812.8 399.4 836.2 454.2C859.6 508.9 877.1 581.9 868.6 651.3C860.2 720.7 799.4 834.2 785.6 870.8";

const SQUARES = [
  "M802.6 802.6L197.4 802.6L197.4 197.4L802.6 197.4Z",
  "M594.4 852.1L147.9 594.4L405.6 147.9L852.1 405.6Z",
  "M419.6 800L200 419.6L580.4 200L800 580.4Z",
  "M313 687L313 313L687 313L687 687Z",
  "M282.4 558.3L441.7 282.4L717.6 441.7L558.3 717.6Z",
  "M314.6 450.3L549.7 314.6L685.4 549.7L450.3 685.4Z",
  "M384.4 384.4L615.6 384.4L615.6 615.6L384.4 615.6Z",
  "M464 365.5L634.5 464L536 634.5L365.5 536Z",
  "M530.7 385.4L614.6 530.7L469.3 614.6L385.4 469.3Z",
];

const TRIANGLES = [
  "M500 108L839.5 696L160.5 696Z",
  "M382 207.9L811.9 543.8L306.1 748.2Z",
  "M324.2 317.9L745.6 438.8L430.2 743.3Z",
  "M314.2 417.3L664.5 380.5L521.3 702.3Z",
  "M336.7 494.3L586.6 361.4L576.7 644.3Z",
  "M376.6 544.9L522.8 370.7L600.6 584.4Z",
];

const TILES = [
  "M286 367.7h428v264.5h-428Z",
  "M286 367.7h264.5v264.5h-264.5Z",
  "M550.5 367.7h163.5v163.5h-163.5Z",
  "M613 531.2h101v101h-101Z",
  "M550.5 569.8h62.4v62.4h-62.4Z",
  "M550.5 531.2h38.6v38.6h-38.6Z",
  "M589.1 531.2h23.9v23.9h-23.9Z",
  "M598.2 555.1h14.7v14.7h-14.7Z",
  "M589.1 560.7h9.1v9.1h-9.1Z",
];

const CHORDS = [
  "M1096.7 626.8L-96.7 373.2",
  "M953.3 908.2L46.7 91.8",
  "M688.5 1080.1L311.5 -80.1",
  "M373.2 1096.7L626.8 -96.7",
  "M91.8 953.3L908.2 46.7",
  "M-80.1 688.5L1080.1 311.5",
];

/** Concentric circles stepping down by φ. */
const CIRCLES = [440, 271.9, 168.1, 103.9, 64.2];

/** Stagger key: each stroke starts drawing a beat after the one before it. */
const beat = (i: number) => ({ "--beat": i }) as React.CSSProperties;

/** `className` lets each screen own its size, placement and opacity; the
 *  figure itself only decides colour and the order things are drawn in. */
export function FibonacciFigure({ className }: { className?: string }) {
  let i = 0;
  return (
    <svg
      className={[styles.plate, className].filter(Boolean).join(" ")}
      viewBox="0 0 1000 1000"
      fill="none"
      aria-hidden="true"
      focusable="false"
    >
      <g className={styles.chord}>
        {CHORDS.map((d) => (
          <path key={d} d={d} pathLength={1} style={beat(i++)} />
        ))}
      </g>
      <g className={styles.hair}>
        {CIRCLES.map((r) => (
          <circle key={r} cx="500" cy="500" r={r} pathLength={1} style={beat(i++)} />
        ))}
        {SQUARES.map((d) => (
          <path key={d} d={d} pathLength={1} style={beat(i++)} />
        ))}
        {TRIANGLES.map((d) => (
          <path key={d} d={d} pathLength={1} style={beat(i++)} />
        ))}
      </g>
      <g className={styles.tile}>
        {TILES.map((d) => (
          <path key={d} d={d} pathLength={1} style={beat(i++)} />
        ))}
      </g>
      <path className={styles.spiral} d={SPIRAL} pathLength={1} style={beat(i++)} />
    </svg>
  );
}
