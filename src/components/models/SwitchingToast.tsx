import { useCatalogStore } from "../../stores/catalog";
import { useModelStore } from "../../stores/model";
import styles from "./SwitchingToast.module.css";

/** The quiet "despertando X…" toast from the chat design — shown for
 *  toast-presented loads (topbar switcher) instead of the full ritual. */
export function SwitchingToast() {
  const status = useModelStore((s) => s.status);
  const loadingId = useModelStore((s) => s.loadingId);
  const presentation = useModelStore((s) => s.loadPresentation);
  const catalogModels = useCatalogStore((s) => s.models);

  if (status !== "loading" || presentation !== "toast" || !loadingId) return null;

  const name = catalogModels.find((m) => m.model.id === loadingId)?.model.name ?? loadingId;

  return (
    <div className={styles.switching} role="status" aria-live="polite">
      <span className="pulse" />
      <span>
        <em>despertando</em> <b className={styles.name}>{name}</b>…
      </span>
    </div>
  );
}
