import { useEffect, useMemo, useState } from "react";
import type { ClassifiedModel, CompatibilityTier } from "../../lib/tauri/bindings";
import { gb } from "../../lib/format";
import { useCatalogStore } from "../../stores/catalog";
import { useDownloadsStore } from "../../stores/downloads";
import { useHardwareStore } from "../../stores/hardware";
import { useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import { ErrorAction, ErrorCard, ErrorLink } from "./ErrorCard";
import styles from "./CatalogPane.module.css";

const TIER_ORDER: CompatibilityTier[] = ["Recommended", "Viable", "Heavy", "NotSupported"];

const TIER_META: Record<
  CompatibilityTier,
  { key: string; roman: string; name: string; gloss: string; chip: string }
> = {
  Recommended: {
    key: "rec",
    roman: "I",
    name: "Recomendados",
    gloss: "cabem na sua máquina sem suar.",
    chip: "recomendados",
  },
  Viable: {
    key: "via",
    roman: "II",
    name: "Viáveis",
    gloss: "rodam bem; talvez peçam paciência em respostas longas.",
    chip: "viáveis",
  },
  Heavy: {
    key: "hea",
    roman: "III",
    name: "Pesados",
    gloss: "rodam, com swap e paciência; mais para ocasiões.",
    chip: "pesados",
  },
  NotSupported: {
    key: "not",
    roman: "IV",
    name: "Não suportados",
    gloss: "precisam de máquina maior — listados para você saber que existem.",
    chip: "não suportados",
  },
};

function DownArrow() {
  return (
    <svg className={styles.actArrow} viewBox="0 0 16 16" fill="none">
      <path
        d="M8 3v9M4 8l4 4 4-4"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function CatalogRow({ classified }: { classified: ClassifiedModel }) {
  const { model, tier } = classified;
  const installed = useModelStore((s) => s.installed.some((m) => m.id === model.id));
  const session = useDownloadsStore((s) => s.session);
  const begin = useDownloadsStore((s) => s.begin);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  const meta = TIER_META[tier];
  const isThisDownload = session?.modelId === model.id && session.phase !== "completed";
  const activePhases = ["starting", "downloading", "verifying"];
  const downloading = isThisDownload && activePhases.includes(session.phase);
  const paused = isThisDownload && (session.phase === "paused" || session.phase === "failed");
  const otherBusy =
    !isThisDownload && session != null && activePhases.includes(session.phase);

  let action: React.ReactNode;
  if (installed) {
    action = <button className={styles.act}>já instalado</button>;
  } else if (downloading) {
    const pct = session.totalBytes > 0 ? (session.downloadedBytes / session.totalBytes) * 100 : 0;
    action = (
      <button className={styles.act} onClick={() => setModelsPane("download")}>
        descendo · {pct.toFixed(0)}%
      </button>
    );
  } else if (paused) {
    action = (
      <button className={styles.act} onClick={() => setModelsPane("download")}>
        retomar download
      </button>
    );
  } else if (tier === "NotSupported") {
    action = (
      <button className={styles.act} disabled>
        fora do alcance
      </button>
    );
  } else {
    action = (
      <button
        className={styles.act}
        disabled={otherBusy}
        title={otherBusy ? "um download por vez — outro modelo já está descendo" : undefined}
        onClick={() => {
          begin(classified);
          setModelsPane("download");
        }}
      >
        {tier === "Heavy" ? "baixar mesmo assim" : "baixar"}
        <DownArrow />
      </button>
    );
  }

  return (
    <article className={styles.crow} data-tier={meta.key} data-installed={installed}>
      <span className={styles.compat} />
      <div className={styles.body}>
        <div className={styles.nameRow}>
          <span className={styles.name}>{model.name}</span>
          <span className={styles.id}>
            {model.publisher} · {model.params_b}b · {model.quantization.toLowerCase()}
          </span>
        </div>
        <p className={styles.rationale}>{model.description}</p>
      </div>
      <div className={styles.params}>
        <span className={styles.param}>
          <b>{model.params_b}B</b>
        </span>
        <span className={styles.param}>
          <b>{model.quantization.toLowerCase()}</b>
        </span>
      </div>
      <span className={styles.size}>
        {gb(model.size_bytes, 1)}
        <span className={styles.sizeSym}>gb</span>
      </span>
      {action}
    </article>
  );
}

export function CatalogPane() {
  const status = useCatalogStore((s) => s.status);
  const models = useCatalogStore((s) => s.models);
  const source = useCatalogStore((s) => s.source);
  const error = useCatalogStore((s) => s.error);
  const refresh = useCatalogStore((s) => s.refresh);
  const detection = useHardwareStore((s) => s.detection);
  const initHardware = useHardwareStore((s) => s.init);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  const [query, setQuery] = useState("");
  const [tierFilter, setTierFilter] = useState<CompatibilityTier | "all">("all");

  useEffect(() => {
    if (status === "idle") void refresh();
    void initHardware();
  }, [status, refresh, initHardware]);

  const counts = useMemo(() => {
    const c = { Recommended: 0, Viable: 0, Heavy: 0, NotSupported: 0 } as Record<
      CompatibilityTier,
      number
    >;
    for (const m of models) c[m.tier] += 1;
    return c;
  }, [models]);

  const visible = useMemo(() => {
    const term = query.trim().toLowerCase();
    return models.filter((m) => {
      if (tierFilter !== "all" && m.tier !== tierFilter) return false;
      if (!term) return true;
      const hay =
        `${m.model.name} ${m.model.publisher} ${m.model.id} ${m.model.quantization} ${m.model.description}`.toLowerCase();
      return hay.includes(term);
    });
  }, [models, query, tierFilter]);

  const machine = detection
    ? `${Math.round(detection.system.memory.total_bytes / 1024 ** 3)} gb · ${detection.system.cpu.physical_cores} núcleos · ${detection.choice.backend}`
    : "—";

  // Offline with nothing cached: the full ceremony error card (Error States V).
  if (status === "error" && models.length === 0) {
    return (
      <section className={styles.column}>
        <div className={styles.inner}>
          <ErrorCard
            tier="warn"
            badge="v · não consegui ler o compêndio"
            code="err.catalog.fetch"
            title="O firmamento está fora do alcance."
            quiet="e não há cópia local para mostrar."
            gloss={
              <>
                O catálogo remoto não respondeu e nenhuma cópia anterior foi guardada nesta
                máquina. Pode ser a rede, pode ser o servidor — confira a conexão e tente de novo.
              </>
            }
            diag={[{ k: "resposta", v: error ?? "sem conexão", italic: true }]}
            actions={
              <>
                <ErrorAction onClick={() => void refresh()}>tentar de novo</ErrorAction>
                <ErrorLink onClick={() => setModelsPane("manager")}>
                  voltar ao ateliê
                </ErrorLink>
              </>
            }
            pulse
          />
        </div>
      </section>
    );
  }

  const grouped = TIER_ORDER.map((tier) => ({
    tier,
    items: visible.filter((m) => m.tier === tier),
  })).filter((g) => g.items.length > 0);

  return (
    <section className={styles.column}>
      <div className={styles.inner}>
        <div className={styles.chead}>
          <div className={styles.cheadLeft}>
            <div className={styles.kicker}>
              <span className={styles.kickerStep}>o compêndio</span>
              <span className={styles.kickerSep}>·</span>
              <span>catálogo remoto</span>
            </div>
            <h1 className={styles.h1}>
              <span>Os modelos do mundo,</span>{" "}
              <span className={styles.h1Quiet}>ainda do outro lado do firmamento.</span>
            </h1>
            <p className={styles.gloss}>
              Tudo que cabe nesta máquina aparece em <em>recomendado</em> ou <em>viável</em>. Os
              pesados rodam, mas vão arrastar; os não-suportados estão listados por integridade,
              não para serem baixados.
            </p>
          </div>
          <div className={styles.machine}>
            sua máquina
            <br />
            <b>{machine}</b>
          </div>
        </div>

        {(status === "error" || source === "cache") && models.length > 0 && (
          <div className={styles.staleNote}>
            o firmamento está fora do alcance — mostrando a última cópia local do compêndio.
            <button className={styles.staleRetry} onClick={() => void refresh()}>
              tentar de novo
            </button>
          </div>
        )}

        <div className={styles.controls}>
          <div className={styles.search}>
            <span className={styles.searchIco}>
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <circle cx="7" cy="7" r="4.5" stroke="currentColor" strokeWidth="1.3" />
                <path d="M10.5 10.5L14 14" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
              </svg>
            </span>
            <input
              type="text"
              placeholder="procurar por nome, autor, quantização…"
              autoComplete="off"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <span className={styles.searchCount}>
              {models.length} {models.length === 1 ? "modelo" : "modelos"}
            </span>
          </div>
          <div className={styles.chips} role="tablist">
            <button
              className={`${styles.chip} ${tierFilter === "all" ? styles.chipActive : ""}`}
              onClick={() => setTierFilter("all")}
            >
              todos <span className={styles.chipCount}>{models.length}</span>
            </button>
            {TIER_ORDER.map((tier) => (
              <button
                key={tier}
                className={`${styles.chip} ${tierFilter === tier ? styles.chipActive : ""}`}
                data-tier={TIER_META[tier].key}
                onClick={() => setTierFilter(tier)}
              >
                <span className={styles.chipDot} />
                {TIER_META[tier].chip} <span className={styles.chipCount}>{counts[tier]}</span>
              </button>
            ))}
          </div>
        </div>

        {status === "loading" && models.length === 0 && (
          <div className={styles.loadingNote}>consultando o firmamento…</div>
        )}

        {grouped.map((group) => (
          <div className={styles.tierGroup} key={group.tier}>
            <div className={styles.tierLabel}>
              <span className={styles.tierRoman}>{TIER_META[group.tier].roman}</span>
              <span className={styles.tierName}>{TIER_META[group.tier].name}</span>
              <span className={styles.tierGloss}>{TIER_META[group.tier].gloss}</span>
            </div>
            {group.items.map((m) => (
              <CatalogRow key={m.model.id} classified={m} />
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}
