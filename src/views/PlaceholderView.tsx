import { AbraxasGlyph } from "../components/shell/AbraxasGlyph";
import styles from "./PlaceholderView.module.css";

/** Quiet centered stand-in for a view that a later phase will build out. */
export function PlaceholderView({ label, note }: { label: string; note: string }) {
  return (
    <section className={styles.placeholder}>
      <div className={styles.glyphWrap}>
        <AbraxasGlyph size={40} />
      </div>
      <span className={styles.label}>{label}</span>
      <span className={styles.note}>{note}</span>
    </section>
  );
}
