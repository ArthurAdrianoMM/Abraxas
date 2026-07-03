import { useEffect } from "react";
import { gb } from "../../lib/format";
import { useCatalogStore } from "../../stores/catalog";
import { useHardwareStore } from "../../stores/hardware";
import { useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import styles from "./ModelSwitcher.module.css";

/** The topbar switcher popover from "Abraxas Chat.html": installed models,
 *  the loaded one marked, quick switch via toast-presented load, and the two
 *  foot links into the Models view. Lightweight — the Models view is the
 *  full surface. */
export function ModelSwitcher() {
  const open = useUiStore((s) => s.switcherOpen);
  const setOpen = useUiStore((s) => s.setSwitcherOpen);
  const openModels = useUiStore((s) => s.openModels);
  const installed = useModelStore((s) => s.installed);
  const loadedId = useModelStore((s) => s.loadedId);
  const status = useModelStore((s) => s.status);
  const load = useModelStore((s) => s.load);
  const catalogModels = useCatalogStore((s) => s.models);
  const refreshCatalog = useCatalogStore((s) => s.refresh);
  const catalogStatus = useCatalogStore((s) => s.status);
  const detection = useHardwareStore((s) => s.detection);
  const initHardware = useHardwareStore((s) => s.init);

  useEffect(() => {
    if (!open) return;
    void initHardware();
    if (catalogStatus === "idle") void refreshCatalog();
  }, [open, initHardware, catalogStatus, refreshCatalog]);

  if (!open) return null;

  const machine = detection
    ? `${Math.round(detection.system.memory.total_bytes / 1024 ** 3)} gb · ${detection.choice.backend}`
    : "—";

  return (
    <>
      <div className={styles.scrim} onClick={() => setOpen(false)} />
      <aside className={styles.popover} role="menu" aria-label="trocar de modelo">
        <div className={styles.head}>
          <span className={styles.kicker}>— qual voz desta vez</span>
          <span className={styles.machine}>
            sua máq · <b>{machine}</b>
          </span>
        </div>

        <div className={styles.list}>
          {installed.length === 0 && (
            <div className={styles.empty}>nenhum codex instalado ainda.</div>
          )}
          {installed.map((m) => {
            const entry = catalogModels.find((c) => c.model.id === m.id)?.model ?? null;
            const active = m.id === loadedId;
            return (
              <button
                key={m.id}
                className={`${styles.item} ${active ? styles.itemActive : ""}`}
                role="menuitem"
                disabled={status === "loading"}
                onClick={() => {
                  setOpen(false);
                  if (!active) void load(m.id, "toast");
                }}
              >
                <span className={styles.itemSeal} style={{ visibility: active ? "visible" : "hidden" }}>
                  ★
                </span>
                <span className={styles.itemBody}>
                  <span className={styles.itemNameRow}>
                    <span className={styles.itemName}>{entry?.name ?? m.id}</span>
                    <span className={styles.itemId}>
                      {entry
                        ? `${entry.publisher} · ${entry.params_b}b · ${entry.quantization.toLowerCase()}`
                        : m.filename}
                    </span>
                  </span>
                  <span className={styles.itemMeta}>
                    {active ? (
                      <>
                        <b>já carregada</b>
                      </>
                    ) : (
                      <>
                        trocar descarrega a atual · <b>desperta em instantes</b>
                      </>
                    )}
                  </span>
                </span>
                <span className={styles.itemRight}>
                  <span className={styles.itemSize}>
                    <b>{gb(m.size_bytes, 1)}</b> gb
                  </span>
                  {active && <span className={styles.itemNow}>agora</span>}
                </span>
              </button>
            );
          })}
        </div>

        <div className={styles.foot}>
          <button
            onClick={() => {
              setOpen(false);
              openModels("catalog");
            }}
          >
            <span>procurar no catálogo</span>
            <span className={styles.footRight}>remoto →</span>
          </button>
          <button
            onClick={() => {
              setOpen(false);
              openModels("manager");
            }}
          >
            <span>ateliê dos modelos</span>
            <span className={styles.footRight}>instalados →</span>
          </button>
        </div>
      </aside>
    </>
  );
}
