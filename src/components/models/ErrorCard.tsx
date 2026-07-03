import styles from "./ErrorCard.module.css";

export interface DiagRow {
  k: string;
  v: React.ReactNode;
  italic?: boolean;
  bordeaux?: boolean;
}

/** The ceremony error card from "Abraxas Error States.html": badge + code,
 *  italic display verdict, hairline rule, prose gloss, diagnostic ledger,
 *  ghost-button actions. */
export function ErrorCard({
  tier = "error",
  badge,
  code,
  title,
  quiet,
  gloss,
  diag,
  actions,
  pulse = false,
}: {
  tier?: "error" | "warn";
  badge: string;
  code: string;
  title: string;
  quiet: string;
  gloss: React.ReactNode;
  diag?: DiagRow[];
  actions?: React.ReactNode;
  /** animated badge dot (in-progress-ish failures like network loss). */
  pulse?: boolean;
}) {
  return (
    <article className={styles.err} data-tier={tier}>
      <div className={styles.top}>
        <span className={styles.badge}>
          <span className={`${styles.dot} ${pulse ? "" : styles.steady}`} />
          {badge}
        </span>
        <span className={styles.code}>{code}</span>
      </div>
      <h2 className={styles.title}>
        <span>{title}</span>
        <span className={styles.quiet}>{quiet}</span>
      </h2>
      <span className={styles.rule} />
      <p className={styles.gloss}>{gloss}</p>
      {diag && diag.length > 0 && (
        <div className={styles.diag}>
          {diag.map((row) => (
            <div className={styles.diagRow} key={row.k}>
              <span className={styles.diagK}>{row.k}</span>
              <span
                className={`${styles.diagV} ${row.italic ? styles.diagItalic : ""} ${
                  row.bordeaux ? styles.diagBordeaux : ""
                }`}
              >
                {row.v}
              </span>
            </div>
          ))}
        </div>
      )}
      {actions && <div className={styles.actions}>{actions}</div>}
    </article>
  );
}

/** Bordeaux-bordered primary action for error cards. */
export function ErrorAction({
  ghost = false,
  onClick,
  children,
}: {
  ghost?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button className={`${styles.gbtn} ${ghost ? styles.gbtnGhost : ""}`} onClick={onClick}>
      {children}
    </button>
  );
}

/** Quiet text-link action for error cards. */
export function ErrorLink({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button className={styles.vlink} onClick={onClick}>
      {children}
    </button>
  );
}
