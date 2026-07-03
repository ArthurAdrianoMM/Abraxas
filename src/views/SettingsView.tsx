import { useCallback, useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { commands, type FontSize, type InferenceBackend } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";
import { ago, gb } from "../lib/format";
import { useConversationsStore } from "../stores/conversations";
import { useDiskStore } from "../stores/disk";
import { useHardwareStore } from "../stores/hardware";
import { useModelStore } from "../stores/model";
import { useSettingsStore } from "../stores/settings";
import styles from "./SettingsView.module.css";

const FONT_SIZES: { value: FontSize; label: string }[] = [
  { value: "compacta", label: "compacta" },
  { value: "comoda", label: "cômoda" },
  { value: "ampla", label: "ampla" },
];

const BACKEND_LABEL: Record<InferenceBackend, { name: string; gloss: string }> = {
  metal: { name: "metal", gloss: "gpu unificada" },
  cuda: { name: "cuda", gloss: "gpu nvidia" },
  vulkan: { name: "vulkan", gloss: "gpu amd · intel" },
  cpu: { name: "cpu", gloss: "processador" },
};

/** Click/drag slider (same interaction as the OrdersDrawer one), but commits
 *  to the backend only on release so a drag is one settings write. */
function Slider({
  value,
  max,
  onChange,
  onCommit,
}: {
  value: number;
  max: number;
  onChange: (v: number) => void;
  onCommit: () => void;
}) {
  const trackRef = useRef<HTMLDivElement>(null);
  const setFromX = useCallback(
    (clientX: number) => {
      const track = trackRef.current;
      if (!track) return;
      const rect = track.getBoundingClientRect();
      const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      onChange(Number((pct * max).toFixed(2)));
    },
    [max, onChange],
  );
  const pct = Math.max(0, Math.min(100, (value / max) * 100));

  return (
    <div
      ref={trackRef}
      className={styles.sliderTrack}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        setFromX(e.clientX);
      }}
      onPointerMove={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) setFromX(e.clientX);
      }}
      onPointerUp={(e) => {
        e.currentTarget.releasePointerCapture(e.pointerId);
        onCommit();
      }}
    >
      <div className={styles.sliderFill} style={{ width: `${pct}%` }} />
      <div className={styles.sliderThumb} style={{ left: `${pct}%` }} />
    </div>
  );
}

function Field({
  name,
  desc,
  children,
}: {
  name: string;
  desc: string;
  children: React.ReactNode;
}) {
  return (
    <div className={styles.field}>
      <div className={styles.label}>
        <span className={styles.labelName}>{name}</span>
        <span className={styles.labelDesc}>{desc}</span>
      </div>
      <div className={styles.value}>{children}</div>
    </div>
  );
}

function Section({
  roman,
  name,
  gloss,
  children,
}: {
  roman: string;
  name: string;
  gloss: string;
  children: React.ReactNode;
}) {
  return (
    <div className={styles.section}>
      <span className={styles.roman}>{roman}</span>
      <div className={styles.sectionBody}>
        <div className={styles.sectionHead}>
          <span className={styles.sectionName}>{name}</span>
          <p className={styles.sectionGloss}>{gloss}</p>
        </div>
        {children}
      </div>
    </div>
  );
}

/** Inline destructive confirmation: danger gbtn → "tem certeza?" verb pair,
 *  same idiom as the sidebar's delete-conversation confirm. */
function DangerAction({
  label,
  confirmLabel,
  busyLabel,
  onConfirm,
}: {
  label: string;
  confirmLabel: string;
  busyLabel: string;
  onConfirm: () => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setConfirming(false);
    setBusy(true);
    setError(null);
    try {
      await onConfirm();
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  if (busy) {
    return <span className={styles.statLine}>{busyLabel}</span>;
  }
  if (confirming) {
    return (
      <div className={styles.actionRow}>
        <span className={styles.confirmLabel}>{confirmLabel}</span>
        <button
          className={`${styles.verbLink} ${styles.verbDanger}`}
          onClick={() => void run()}
        >
          sim, apagar
        </button>
        <span className={styles.verbSep}>·</span>
        <button className={styles.verbLink} onClick={() => setConfirming(false)}>
          manter
        </button>
      </div>
    );
  }
  return (
    <>
      <button
        className={`${styles.gbtn} ${styles.gbtnDanger}`}
        onClick={() => setConfirming(true)}
      >
        {label}
      </button>
      {error && <span className={`${styles.statLine} ${styles.statBad}`}>{error}</span>}
    </>
  );
}

export function SettingsView() {
  const settings = useSettingsStore((s) => s.settings);
  const save = useSettingsStore((s) => s.save);
  const refreshSettings = useSettingsStore((s) => s.refresh);
  const initSettings = useSettingsStore((s) => s.init);
  const settingsError = useSettingsStore((s) => s.error);

  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);

  const installed = useModelStore((s) => s.installed);
  const initModel = useModelStore((s) => s.init);
  const refreshInstalled = useModelStore((s) => s.refreshInstalled);

  const detection = useHardwareStore((s) => s.detection);
  const redetecting = useHardwareStore((s) => s.redetecting);
  const initHardware = useHardwareStore((s) => s.init);
  const redetect = useHardwareStore((s) => s.redetect);

  const clearAllConversations = useConversationsStore((s) => s.clearAll);
  const loadConversations = useConversationsStore((s) => s.load);
  const startNew = useConversationsStore((s) => s.startNew);

  const [version, setVersion] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  // Local drafts for the generation params so a slider drag or half-typed
  // number doesn't write on every keystroke.
  const [temp, setTemp] = useState(0.8);
  const [topP, setTopP] = useState(0.95);
  const [maxTokens, setMaxTokens] = useState("");
  const [seed, setSeed] = useState("");

  useEffect(() => {
    void initSettings();
    void initModel();
    void initHardware();
    void refreshDisk();
    void unwrap(commands.appInfo())
      .then((info) => setVersion(info.version))
      .catch(() => setVersion(null));
  }, [initSettings, initModel, initHardware, refreshDisk]);

  useEffect(() => {
    if (!settings) return;
    setTemp(settings.default_temperature);
    setTopP(settings.default_top_p);
    setMaxTokens(String(settings.default_max_completion_tokens));
    setSeed(settings.default_seed === null ? "" : String(settings.default_seed));
  }, [settings]);

  const commitMaxTokens = () => {
    const n = parseInt(maxTokens, 10);
    if (Number.isFinite(n) && n > 0) {
      void save({ default_max_completion_tokens: n });
    } else if (settings) {
      setMaxTokens(String(settings.default_max_completion_tokens));
    }
  };

  const commitSeed = () => {
    if (seed.trim() === "") {
      void save({ default_seed: null });
      return;
    }
    const n = parseInt(seed, 10);
    if (Number.isFinite(n) && n >= 0) {
      void save({ default_seed: n });
    } else if (settings) {
      setSeed(settings.default_seed === null ? "" : String(settings.default_seed));
    }
  };

  const openModelsFolder = async () => {
    if (!usage) return;
    setFolderError(null);
    try {
      await revealItemInDir(usage.models_dir);
    } catch {
      setFolderError("a pasta ainda não existe — nada foi baixado até aqui.");
    }
  };

  const runIntegrityCheck = async () => {
    setChecking(true);
    try {
      await unwrap(commands.verifyInstalledModels());
      await refreshSettings();
    } finally {
      setChecking(false);
    }
  };

  const burnEverything = async () => {
    await unwrap(commands.clearAllData());
    startNew();
    await Promise.all([loadConversations(), refreshInstalled(), refreshSettings(), refreshDisk()]);
  };

  const integrity = settings?.last_integrity_check ?? null;
  const totalModelBytes = installed.reduce((acc, m) => acc + m.size_bytes, 0);
  const backend = detection ? BACKEND_LABEL[detection.choice.backend] : null;

  return (
    <section className={styles.column}>
      <div className={styles.inner}>
        <div className={styles.shead}>
          <div className={styles.kicker}>
            <span className={styles.kickerStep}>preferências</span>
            <span className={styles.kickerSep}>·</span>
            <span>as ordens da casa</span>
          </div>
          <h1 className={styles.h1}>
            <span>As preferências.</span>{" "}
            <span className={styles.h1Quiet}>cinco capítulos breves.</span>
          </h1>
          {settingsError && (
            <span className={`${styles.statLine} ${styles.statBad}`}>{settingsError}</span>
          )}
        </div>

        {/* I · aparência */}
        <Section
          roman="I"
          name="A aparência da página"
          gloss="A casa nasceu em pergaminho noturno; o que se ajusta é a medida da letra."
        >
          <Field name="tamanho da letra" desc="para leitura longa.">
            <div className={styles.radioRow}>
              {FONT_SIZES.map((f) => {
                const active = (settings?.font_size ?? "comoda") === f.value;
                return (
                  <button
                    key={f.value}
                    className={`${styles.radioChip} ${active ? styles.radioChipActive : ""}`}
                    onClick={() => void save({ font_size: f.value })}
                  >
                    <span className={styles.radioDot} />
                    {f.label}
                  </button>
                );
              })}
            </div>
          </Field>
        </Section>

        {/* II · pasta dos modelos */}
        <Section
          roman="II"
          name="A pasta dos modelos"
          gloss="Onde os codices ficam guardados neste computador."
        >
          <Field name="caminho" desc="absoluto, no seu sistema.">
            <div className={styles.ti}>
              <input
                className={styles.tiInput}
                type="text"
                readOnly
                value={usage?.models_dir ?? "…"}
              />
              <button
                className={styles.iconBtnInline}
                title="abrir pasta"
                onClick={() => void openModelsFolder()}
              >
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M2 5.5V4a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1V11a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V5.5z"
                    stroke="currentColor"
                    strokeWidth="1.2"
                  />
                </svg>
              </button>
            </div>
            <span className={styles.statLine}>
              {installed.length} {installed.length === 1 ? "modelo" : "modelos"} ·{" "}
              {gb(totalModelBytes, 1)} gb
              {usage && usage.total_bytes > 0 && <> · disponível: {gb(usage.free_bytes, 0)} gb</>}
            </span>
            {folderError && (
              <span className={`${styles.statLine} ${styles.statBad}`}>{folderError}</span>
            )}
          </Field>

          <Field name="conferir integridade" desc="recalcular hashes dos arquivos baixados.">
            <div className={styles.actionRow}>
              <button
                className={styles.verbLink}
                disabled={checking || installed.length === 0}
                onClick={() => void runIntegrityCheck()}
              >
                {checking ? "conferindo…" : "conferir agora"}
              </button>
              <span className={styles.verbSep}>·</span>
              <span
                className={`${styles.statLine} ${
                  integrity && integrity.corrupt.length > 0 ? styles.statBad : ""
                }`}
              >
                {installed.length === 0
                  ? "nada na estante para conferir"
                  : integrity
                    ? `última: ${ago(integrity.at)} · ${
                        integrity.corrupt.length === 0
                          ? "íntegro"
                          : `${integrity.corrupt.length} ${
                              integrity.corrupt.length === 1 ? "corrompido" : "corrompidos"
                            }`
                      }`
                    : "nunca conferido"}
              </span>
            </div>
          </Field>
        </Section>

        {/* III · parâmetros de geração */}
        <Section
          roman="III"
          name="A medida do verbo"
          gloss="Os parâmetros que entram em toda conversa nova. Cada conversa pode sobrescrever os seus."
        >
          <Field name="temperatura" desc="quão dilatado o modelo se permite ser.">
            <div className={styles.sliderRow}>
              <div className={styles.sliderTop}>
                <span>movimento</span>
                <span className={styles.reading}>{temp.toFixed(2)}</span>
              </div>
              <Slider
                value={temp}
                max={2}
                onChange={setTemp}
                onCommit={() => void save({ default_temperature: temp })}
              />
              <div className={styles.sliderBounds}>
                <span>0 · pedra</span>
                <span>2 · febre</span>
              </div>
            </div>
          </Field>

          <Field name="top-p" desc="fração de probabilidade considerada por token.">
            <div className={styles.sliderRow}>
              <div className={styles.sliderTop}>
                <span>amplitude</span>
                <span className={styles.reading}>{topP.toFixed(2)}</span>
              </div>
              <Slider
                value={topP}
                max={1}
                onChange={setTopP}
                onCommit={() => void save({ default_top_p: topP })}
              />
              <div className={styles.sliderBounds}>
                <span>0</span>
                <span>1</span>
              </div>
            </div>
          </Field>

          <Field name="máximo de tokens" desc="o quanto pode dizer numa só resposta.">
            <div className={styles.ti} style={{ maxWidth: 200 }}>
              <input
                className={`${styles.tiInput} ${styles.tiNum}`}
                type="text"
                inputMode="numeric"
                value={maxTokens}
                onChange={(e) => setMaxTokens(e.target.value)}
                onBlur={commitMaxTokens}
                onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              />
              <span className={styles.tiSuffix}>tokens</span>
            </div>
          </Field>

          <Field
            name="semente"
            desc="para reproduzir uma mesma resposta; em branco, usa a semente padrão."
          >
            <div className={styles.ti} style={{ maxWidth: 260 }}>
              <input
                className={`${styles.tiInput} ${styles.tiNum}`}
                type="text"
                inputMode="numeric"
                value={seed}
                placeholder="padrão · ex.: 365"
                onChange={(e) => setSeed(e.target.value)}
                onBlur={commitSeed}
                onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              />
              <button
                className={styles.iconBtnInline}
                title="voltar ao padrão"
                onClick={() => {
                  setSeed("");
                  void save({ default_seed: null });
                }}
              >
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M3 8a5 5 0 0 1 9-3M13 8a5 5 0 0 1-9 3M12 3v3h-3M4 13v-3h3"
                    stroke="currentColor"
                    strokeWidth="1.2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </button>
            </div>
          </Field>
        </Section>

        {/* IV · instrumento */}
        <Section
          roman="IV"
          name="O instrumento"
          gloss="Se trocou de máquina ou ligou outro periférico, refaça o exame."
        >
          <Field name="backend" desc="como o modelo é executado — escolhido pela casa.">
            <div className={styles.backendBox}>
              {backend ? (
                <span>
                  <span className={styles.backendName}>{backend.name}</span> · {backend.gloss}
                </span>
              ) : (
                <span>examinando…</span>
              )}
            </div>
            {detection && (
              <span className={styles.statLine}>
                detectado · {detection.system.cpu.physical_cores} núcleos ·{" "}
                {gb(detection.system.memory.total_bytes, 0)} gb de memória
              </span>
            )}
          </Field>

          <Field name="refazer o exame" desc="recalcula o que esta máquina aguenta.">
            <div className={styles.actionRow}>
              <button
                className={styles.gbtn}
                disabled={redetecting}
                onClick={() => void redetect()}
              >
                {redetecting ? "examinando…" : "examinar de novo"}
                <svg width="11" height="11" viewBox="0 0 16 16" fill="none">
                  <path
                    d="M3 8h10M9 4l4 4-4 4"
                    stroke="currentColor"
                    strokeWidth="1.4"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </button>
              {detection && !redetecting && (
                <span className={styles.statLine}>
                  {detection.from_cache
                    ? `exame guardado · ${ago(detection.detected_at)}`
                    : `examinado ${ago(detection.detected_at)}`}
                </span>
              )}
            </div>
          </Field>
        </Section>

        {/* V · sobre + ações irreversíveis */}
        <Section
          roman="V"
          name="A casa, em poucas linhas"
          gloss="Versão, créditos, e as ações irreversíveis que se faz à meia-noite."
        >
          <Field name="sobre" desc="o que está sendo usado.">
            <div className={styles.about}>
              <p>
                <span className={styles.aboutKey}>versão</span>
                <b>Abraxas {version ?? "—"}</b>
              </p>
              <p>
                <span className={styles.aboutKey}>tempo de execução</span>
                llama.cpp{backend ? <> · {backend.name}</> : null}
              </p>
              <p>
                <span className={styles.aboutKey}>licença</span>
                <em>uso pessoal e contemplativo · MIT</em>
              </p>
              <p className={styles.aboutQuote}>
                “O pássaro luta para sair do ovo. O ovo é o mundo.”
              </p>
            </div>
          </Field>

          <Field name="apagar conversas" desc="remove o histórico, mantém modelos. não há volta.">
            <DangerAction
              label="apagar todo o histórico"
              confirmLabel="todo o histórico, para sempre?"
              busyLabel="apagando…"
              onConfirm={clearAllConversations}
            />
          </Field>

          <Field name="apagar tudo" desc="conversas, modelos, preferências. a casa vai esquecer.">
            <DangerAction
              label="queimar tudo"
              confirmLabel="conversas, modelos e preferências — tudo?"
              busyLabel="queimando…"
              onConfirm={burnEverything}
            />
          </Field>
        </Section>
      </div>
    </section>
  );
}
