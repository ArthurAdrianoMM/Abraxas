import { useEffect, useMemo, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { InstalledModel, ModelEntry } from "../../lib/tauri/bindings";
import { describeError } from "../../lib/tauri/result";
import { ago, contextK, gb, roman } from "../../lib/format";
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
          ? "esta voz está desperta — desperte outra antes de remover."
          : describeError(e),
      );
    }
  };

  return (
    <article className={styles.row} data-loaded={isLoaded} data-default={isDefault}>
      <span className={styles.roman}>{roman(index)}</span>
      <span className={styles.sealMark} title={isLoaded ? "voz desperta" : undefined}>
        <span className={styles.star}>★</span>
      </span>
      <div className={styles.body}>
        <div className={styles.nameRow}>
          <span className={styles.name}>{entry?.name ?? installed.id}</span>
          {isDefault && (
            <span className={styles.defaultTag} title="acorda com o app">
              padrão
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
                <b>{entry.params_b}B</b> parâmetros
              </span>
              <span className={styles.tag}>
                <b>{entry.quantization.toLowerCase()}</b> · quantização
              </span>
              <span className={styles.tag}>
                <b>{contextK(entry.context_length)}</b> · contexto
              </span>
            </>
          )}
        </div>
        <div className={styles.meta}>
          <span>
            instalado · <b>{ago(installed.installed_at)}</b>
          </span>
        </div>
        {removeError && (
          <div className={styles.rowNotice}>
            {removeError}
            <button className={styles.rowNoticeDismiss} onClick={() => setRemoveError(null)}>
              ok
            </button>
          </div>
        )}
      </div>
      <div className={styles.actions}>
        <span className={styles.size}>
          {gb(installed.size_bytes, 1)}
          <span className={styles.sizeSym}>gb</span>
        </span>
        {confirming ? (
          <div className={styles.verbs}>
            <span className={styles.confirmLabel}>remover este codex?</span>
            <button className={`${styles.verbLink} ${styles.verbDanger}`} onClick={() => void handleRemove()}>
              remover
            </button>
            <span className={styles.verbSep}>·</span>
            <button className={styles.verbLink} onClick={() => setConfirming(false)}>
              manter
            </button>
          </div>
        ) : (
          <div className={styles.verbs}>
            <button
              className={styles.verbLink}
              disabled={isLoaded || loading}
              onClick={() => void load(installed.id, "ritual")}
            >
              {isLoaded ? "desperta agora" : "despertar"}
            </button>
            <span className={styles.verbSep}>·</span>
            <button
              className={styles.verbLink}
              disabled={isDefault}
              title={isDefault ? undefined : "acordar esta voz ao abrir o app"}
              onClick={() => void saveSettings({ default_model_id: installed.id })}
            >
              {isDefault ? "já é padrão" : "tornar padrão"}
            </button>
            <span className={styles.verbSep}>·</span>
            <button
              className={styles.verbLink}
              onClick={() => void revealItemInDir(installed.path).catch(() => undefined)}
            >
              abrir pasta
            </button>
            <span className={styles.verbSep}>·</span>
            <button
              className={`${styles.verbLink} ${styles.verbDanger}`}
              disabled={isLoaded}
              title={isLoaded ? "desperte outra voz antes de remover esta" : undefined}
              onClick={() => setConfirming(true)}
            >
              remover
            </button>
          </div>
        )}
      </div>
    </article>
  );
}

export function ManagerPane() {
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
            <span className={styles.kickerStep}>o ateliê</span>
            <span className={styles.kickerSep}>·</span>
            <span>modelos instalados</span>
          </div>
          <h1 className={styles.h1}>
            <span>Os modelos da casa.</span>{" "}
            <span className={styles.h1Quiet}>leves, médios, e os que pesam.</span>
          </h1>
          <p className={styles.gloss}>
            Cada modelo é um codex carregado do firmamento e guardado neste computador.
            {installed.length > 0
              ? ` ${installed.length === 1 ? "Um está pronto" : `${installed.length} estão prontos`} para conversar; nada sai daqui sem você pedir.`
              : " A estante ainda está vazia — procure o compêndio para trazer o primeiro."}
          </p>
        </div>

        {loadError && (
          <ErrorCard
            badge="i · não carregou"
            code="err.load.weights"
            title="O oráculo não acordou."
            quiet="o arquivo está aqui, mas recusou."
            gloss={
              <>
                O modelo <em>{loadingId ?? "escolhido"}</em> não pôde ser lido para a memória. O
                arquivo continua no disco; tente de novo ou desperte outra voz.
              </>
            }
            diag={[{ k: "causa provável", v: loadError, italic: true }]}
            actions={
              <>
                {loadingId && (
                  <ErrorAction
                    onClick={() => {
                      dismissError();
                      void load(loadingId, "ritual");
                    }}
                  >
                    tentar de novo
                  </ErrorAction>
                )}
                <ErrorLink onClick={dismissError}>dispensar</ErrorLink>
              </>
            }
          />
        )}

        {installed.length > 0 && (
          <div className={styles.disk} role="group" aria-label="Espaço em disco">
            <div className={styles.diskTop}>
              <div className={styles.diskLhs}>
                <b>{gb(totalBytes, 1)} GB</b> consagrados aos modelos
                {usage && usage.total_bytes > 0 && (
                  <span className={styles.diskFree}>
                    {" "}
                    · {gb(usage.free_bytes, 0)} GB livres no disco
                  </span>
                )}
              </div>
              <div className={styles.diskRhs}>
                {usage && usage.total_bytes > 0
                  ? `${((totalBytes / usage.total_bytes) * 100).toFixed(1).replace(".", ",")}% do disco`
                  : `${installed.length} ${installed.length === 1 ? "codex" : "codices"}`}
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
              — os codices · {installed.length} {installed.length === 1 ? "instalado" : "instalados"}
            </span>
            <span className={styles.entriesHeadRight}>tamanho</span>
          </div>

          <div className={styles.entries}>
            {installed.length === 0 && modelStatus !== "unknown" && modelStatus !== "initializing" && (
              <div className={styles.empty}>
                nenhum codex na estante ainda — o compêndio remoto tem o que baixar.
              </div>
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
                <span>procurar no catálogo</span>
              </span>
              <span className={styles.catalogRight}>
                <span>compêndio remoto</span>
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
