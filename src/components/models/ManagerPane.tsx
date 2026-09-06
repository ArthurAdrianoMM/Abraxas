import { useEffect, useMemo, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { InstalledModel, ModelEntry } from "../../lib/tauri/bindings";
import { describeError } from "../../lib/tauri/result";
import { useFormat, useT } from "../../lib/i18n";
import { useCatalogStore } from "../../stores/catalog";
import { useDiskStore } from "../../stores/disk";
import { useModelStore } from "../../stores/model";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";
import { ErrorAction, ErrorCard, ErrorLink } from "./ErrorCard";
import styles from "./ManagerPane.module.css";

function ManagerRow({
  installed,
  entry,
  index,
}: {
  installed: InstalledModel;
  /** Catalog entry for richer metadata; null when the catalog is unreachable. */
  entry: ModelEntry | null;
  index: number;
}) {
  const t = useT().manager;
  const f = useFormat();
  const loadedId = useModelStore((s) => s.loadedId);
  const status = useModelStore((s) => s.status);
  const load = useModelStore((s) => s.load);
  const remove = useModelStore((s) => s.remove);
  const defaultModelId = useSettingsStore((s) => s.settings?.default_model_id ?? null);
  const saveSettings = useSettingsStore((s) => s.save);
  const [confirming, setConfirming] = useState(false);
  const [removeError, setRemoveError] = useState<string | null>(null);

  const isLoaded = loadedId === installed.id;
  const isDefault = defaultModelId === installed.id;
  const loading = status === "loading";

  const handleRemove = async () => {
    setConfirming(false);
    try {
      await remove(installed.id);
      // A removed model can't be the startup default anymore.
      if (isDefault) void saveSettings({ default_model_id: null });
    } catch (e) {
      const kind = (e as { kind?: string })?.kind;
      setRemoveError(
        kind === "ModelLoaded"
          ? t.removeLoaded
          : describeError(e),
      );
    }
  };

  return (
    <article className={styles.row} data-loaded={isLoaded} data-default={isDefault}>
      <span className={styles.roman}>{f.roman(index)}</span>
      <span className={styles.sealMark} title={isLoaded ? t.awakeTitle : undefined}>
        <span className={styles.star}>★</span>
      </span>
      <div className={styles.body}>
        <div className={styles.nameRow}>
          <span className={styles.name}>{entry?.name ?? installed.id}</span>
          {isDefault && (
            <span className={styles.defaultTag} title={t.defaultTagTitle}>
              {t.defaultTag}
            </span>
          )}
          <span className={styles.id}>
            {entry
              ? `${entry.publisher} · ${installed.id} · ${entry.quantization.toLowerCase()}`
              : installed.filename}
          </span>
        </div>
        <div className={styles.tags}>
          {entry && (
            <>
              <span className={styles.tag}>
                <b>{entry.params_b}B</b> {t.params}
              </span>
              <span className={styles.tag}>
                <b>{entry.quantization.toLowerCase()}</b> · {t.quantization}
              </span>
              <span className={styles.tag}>
                <b>{f.contextK(entry.context_length)}</b> · {t.context}
              </span>
            </>
          )}
        </div>
        <div className={styles.meta}>
          <span>
            {t.installedAt} · <b>{f.ago(installed.installed_at)}</b>
          </span>
        </div>
        {removeError && (
          <div className={styles.rowNotice}>
            {removeError}
            <button className={styles.rowNoticeDismiss} onClick={() => setRemoveError(null)}>
              {t.dismiss}
            </button>
          </div>
        )}
      </div>
      <div className={styles.actions}>
        <span className={styles.size}>
          {f.gb(installed.size_bytes, 1)}
          <span className={styles.sizeSym}>gb</span>
        </span>
        {confirming ? (
          <div className={styles.verbs}>
            <span className={styles.confirmLabel}>{t.confirmRemove}</span>
            <button className={`${styles.verbLink} ${styles.verbDanger}`} onClick={() => void handleRemove()}>
              {t.remove}
            </button>
            <span className={styles.verbSep}>·</span>
            <button className={styles.verbLink} onClick={() => setConfirming(false)}>
              {t.keep}
            </button>
          </div>
        ) : (
          <div className={styles.verbs}>
            <button
              className={styles.verbLink}
              disabled={isLoaded || loading}
              onClick={() => void load(installed.id, "ritual")}
            >
              {isLoaded ? t.awakeNow : t.awaken}
            </button>
            <span className={styles.verbSep}>·</span>
            <button
              className={styles.verbLink}
              disabled={isDefault}
              title={isDefault ? undefined : t.makeDefaultTitle}
              onClick={() => void saveSettings({ default_model_id: installed.id })}
            >
              {isDefault ? t.isDefault : t.makeDefault}
            </button>
            <span className={styles.verbSep}>·</span>
            <button
              className={styles.verbLink}
              onClick={() => void revealItemInDir(installed.path).catch(() => undefined)}
            >
              {t.openFolder}
            </button>
            <span className={styles.verbSep}>·</span>
            <button
              className={`${styles.verbLink} ${styles.verbDanger}`}
              disabled={isLoaded}
              title={isLoaded ? t.removeBlocked : undefined}
              onClick={() => setConfirming(true)}
            >
              {t.remove}
            </button>
          </div>
        )}
      </div>
    </article>
  );
}

export function ManagerPane() {
  const t = useT().manager;
  const f = useFormat();
  const installed = useModelStore((s) => s.installed);
  const modelStatus = useModelStore((s) => s.status);
  const loadError = useModelStore((s) => s.error);
  const loadingId = useModelStore((s) => s.loadingId);
  const dismissError = useModelStore((s) => s.dismissError);
  const load = useModelStore((s) => s.load);
  const catalogModels = useCatalogStore((s) => s.models);
  const setModelsPane = useUiStore((s) => s.setModelsPane);
  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);
  const initSettings = useSettingsStore((s) => s.init);

  useEffect(() => {
    void refreshDisk();
    void initSettings();
  }, [refreshDisk, initSettings, installed.length]);

  const entriesById = useMemo(() => {
    const map = new Map<string, ModelEntry>();
    for (const m of catalogModels) map.set(m.model.id, m.model);
    return map;
  }, [catalogModels]);

  const totalBytes = installed.reduce((acc, m) => acc + m.size_bytes, 0);

  return (
    <section className={styles.column}>
      <div className={styles.inner}>
        <div className={styles.mhead}>
          <div className={styles.kicker}>
            <span className={styles.kickerStep}>{t.kickerStep}</span>
            <span className={styles.kickerSep}>·</span>
            <span>{t.kickerSub}</span>
          </div>
          <h1 className={styles.h1}>
            <span>{t.h1Lead}</span> <span className={styles.h1Quiet}>{t.h1Quiet}</span>
          </h1>
          <p className={styles.gloss}>
            {t.glossLead}
            {installed.length > 0 ? t.glossReady(installed.length) : t.glossEmpty}
          </p>
        </div>

        {loadError && (
          <ErrorCard
            badge={t.loadFailed.badge}
            code="err.load.weights"
            title={t.loadFailed.title}
            quiet={t.loadFailed.quiet}
            gloss={t.loadFailed.gloss(loadingId ?? t.loadFailed.chosen)}
            diag={[{ k: t.loadFailed.diagKey, v: loadError, italic: true }]}
            actions={
              <>
                {loadingId && (
                  <ErrorAction
                    onClick={() => {
                      dismissError();
                      void load(loadingId, "ritual");
                    }}
                  >
                    {t.loadFailed.retry}
                  </ErrorAction>
                )}
                <ErrorLink onClick={dismissError}>{t.loadFailed.dismiss}</ErrorLink>
              </>
            }
          />
        )}

        {installed.length > 0 && (
          <div className={styles.disk} role="group" aria-label={t.diskAria}>
            <div className={styles.diskTop}>
              <div className={styles.diskLhs}>
                <b>{f.gb(totalBytes, 1)} GB</b> {t.consecrated}
                {usage && usage.total_bytes > 0 && (
                  <span className={styles.diskFree}>{t.freeOnDisk(f.gb(usage.free_bytes, 0))}</span>
                )}
              </div>
              <div className={styles.diskRhs}>
                {usage && usage.total_bytes > 0
                  ? t.diskPct(f.decimal((totalBytes / usage.total_bytes) * 100, 1))
                  : `${installed.length} ${t.codexCount(installed.length)}`}
              </div>
            </div>
            {usage && usage.total_bytes > 0 && (
              <div className={styles.diskMeter} aria-hidden="true">
                <div
                  className={styles.diskMeterFilled}
                  style={{ width: `${Math.min(100, (totalBytes / usage.total_bytes) * 100)}%` }}
                />
              </div>
            )}
          </div>
        )}

        <div>
          <div className={styles.entriesHead}>
            <span className={styles.entriesHeadLeft}>
              {t.entriesHead(installed.length)}
            </span>
            <span className={styles.entriesHeadRight}>{t.sizeCol}</span>
          </div>

          <div className={styles.entries}>
            {installed.length === 0 && modelStatus !== "unknown" && modelStatus !== "initializing" && (
              <div className={styles.empty}>{t.empty}</div>
            )}
            {installed.map((m, i) => (
              <ManagerRow
                key={m.id}
                installed={m}
                entry={entriesById.get(m.id) ?? null}
                index={i}
              />
            ))}

            <button className={styles.catalogRow} onClick={() => setModelsPane("catalog")}>
              <span className={styles.catalogLeft}>
                <span className={styles.catalogPlus}>+</span>
                <span>{t.browseCatalog}</span>
              </span>
              <span className={styles.catalogRight}>
                <span>{t.remoteCatalog}</span>
                <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M3 8h10M9 4l4 4-4 4"
                    stroke="currentColor"
                    strokeWidth="1.3"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </span>
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
