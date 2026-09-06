import { useT } from "../../lib/i18n";
import { useCatalogStore } from "../../stores/catalog";
import { displayNameOf, useModelStore } from "../../stores/model";
import styles from "./SwitchingToast.module.css";

/** The quiet "despertando X…" toast from the chat design — shown for
 *  toast-presented loads (topbar switcher) instead of the full ritual. */
export function SwitchingToast() {
  const t = useT().shell;
  const status = useModelStore((s) => s.status);
  const loadingId = useModelStore((s) => s.loadingId);
  const presentation = useModelStore((s) => s.loadPresentation);
  const catalogModels = useCatalogStore((s) => s.models);
  const installed = useModelStore((s) => s.installed);

  if (status !== "loading" || presentation !== "toast" || !loadingId) return null;

  const name = displayNameOf(
    installed.find((m) => m.id === loadingId),
    catalogModels.find((m) => m.model.id === loadingId)?.model,
    loadingId,
  );

  return (
    <div className={styles.switching} role="status" aria-live="polite">
      <span className="pulse" />
      <span>
        <em>{t.awakening}</em> <b className={styles.name}>{name}</b>…
      </span>
    </div>
  );
}
