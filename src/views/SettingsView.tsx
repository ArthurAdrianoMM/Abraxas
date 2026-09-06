import { useCallback, useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { commands, type FontSize, type Locale } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";
import { useFormat, useLocale, useT, LOCALES, LOCALE_NAMES } from "../lib/i18n";
import { useConversationsStore } from "../stores/conversations";
import { useDiskStore } from "../stores/disk";
import { useHardwareStore } from "../stores/hardware";
import { useModelStore } from "../stores/model";
import { useSettingsStore } from "../stores/settings";
import styles from "./SettingsView.module.css";

/** Persisted identifiers, not copy: these are the Rust `FontSize` variants
 *  stored in SQLite. Their visible labels come from the dictionary. */
const FONT_SIZES: FontSize[] = ["compacta", "comoda", "ampla"];

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
  const t = useT().settings;
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
          {t.confirmYes}
        </button>
        <span className={styles.verbSep}>·</span>
        <button className={styles.verbLink} onClick={() => setConfirming(false)}>
          {t.confirmNo}
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
  const t = useT().settings;
  const f = useFormat();
  const locale = useLocale();
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
    const trimmed = maxTokens.trim();
    const n = Number(trimmed);
    if (/^\d+$/.test(trimmed) && Number.isSafeInteger(n) && n > 0 && n <= 4_294_967_295) {
      void save({ default_max_completion_tokens: n });
    } else if (settings) {
      setMaxTokens(String(settings.default_max_completion_tokens));
    }
  };

  const commitSeed = () => {
    const trimmed = seed.trim();
    if (trimmed === "") {
      void save({ default_seed: null });
      return;
    }
    const n = Number(trimmed);
    if (/^\d+$/.test(trimmed) && Number.isSafeInteger(n)) {
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
      setFolderError(t.folder.folderMissing);
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
    try {
      await unwrap(commands.clearAllData());
    } catch (e) {
      // `PartialClear` carries the leftover file list as its message; the
      // sentence around it is ours to phrase, in the user's language.
      if ((e as { kind?: string })?.kind === "PartialClear") {
        const files = describeError(e);
        throw new Error(t.about.partialClear(files.split(", ").length, files));
      }
      throw e;
    }
    startNew();
    await Promise.all([loadConversations(), refreshInstalled(), refreshSettings(), refreshDisk()]);
  };

  const integrity = settings?.last_integrity_check ?? null;
  const totalModelBytes = installed.reduce((acc, m) => acc + m.size_bytes, 0);
  const backend = detection ? detection.choice.backend : null;

  return (
    <section className={styles.column}>
      <div className={styles.inner}>
        <div className={styles.shead}>
          <div className={styles.kicker}>
            <span className={styles.kickerStep}>{t.kickerStep}</span>
            <span className={styles.kickerSep}>·</span>
            <span>{t.kickerSub}</span>
          </div>
          <h1 className={styles.h1}>
            <span>{t.h1Lead}</span> <span className={styles.h1Quiet}>{t.h1Quiet}</span>
          </h1>
          {settingsError && (
            <span className={`${styles.statLine} ${styles.statBad}`}>{settingsError}</span>
          )}
        </div>

        {/* I · appearance */}
        <Section roman="I" name={t.appearance.name} gloss={t.appearance.gloss}>
          <Field name={t.appearance.fontSize} desc={t.appearance.fontSizeDesc}>
            <div className={styles.radioRow}>
              {FONT_SIZES.map((size) => {
                const active = (settings?.font_size ?? "comoda") === size;
                return (
                  <button
                    key={size}
                    className={`${styles.radioChip} ${active ? styles.radioChipActive : ""}`}
                    onClick={() => void save({ font_size: size })}
                  >
                    <span className={styles.radioDot} />
                    {t.fontSizes[size]}
                  </button>
                );
              })}
            </div>
          </Field>

          <Field name={t.appearance.language} desc={t.appearance.languageDesc}>
            <div className={styles.radioRow}>
              {LOCALES.map((code) => (
                <button
                  key={code}
                  className={`${styles.radioChip} ${
                    locale === code ? styles.radioChipActive : ""
                  }`}
                  onClick={() => void save({ locale: code as Locale })}
                >
                  <span className={styles.radioDot} />
                  {LOCALE_NAMES[code]}
                </button>
              ))}
            </div>
          </Field>
        </Section>

        {/* II · models folder */}
        <Section roman="II" name={t.folder.name} gloss={t.folder.gloss}>
          <Field name={t.folder.path} desc={t.folder.pathDesc}>
            <div className={styles.ti}>
              <input
                className={styles.tiInput}
                type="text"
                readOnly
                value={usage?.models_dir ?? "…"}
              />
              <button
                className={styles.iconBtnInline}
                title={t.folder.openFolder}
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
              {installed.length} {t.folder.count(installed.length)} ·{" "}
              {f.gb(totalModelBytes, 1)} gb
              {usage && usage.total_bytes > 0 && t.folder.available(f.gb(usage.free_bytes, 0))}
            </span>
            {folderError && (
              <span className={`${styles.statLine} ${styles.statBad}`}>{folderError}</span>
            )}
          </Field>

          <Field name={t.folder.integrity} desc={t.folder.integrityDesc}>
            <div className={styles.actionRow}>
              <button
                className={styles.verbLink}
                disabled={checking || installed.length === 0}
                onClick={() => void runIntegrityCheck()}
              >
                {checking ? t.folder.checking : t.folder.checkNow}
              </button>
              <span className={styles.verbSep}>·</span>
              <span
                className={`${styles.statLine} ${
                  integrity && integrity.corrupt.length > 0 ? styles.statBad : ""
                }`}
              >
                {installed.length === 0
                  ? t.folder.nothingToCheck
                  : integrity
                    ? t.folder.last(
                        f.ago(integrity.at),
                        integrity.corrupt.length === 0
                          ? t.folder.intact
                          : t.folder.corrupt(integrity.corrupt.length),
                      )
                    : t.folder.never}
              </span>
            </div>
          </Field>
        </Section>

        {/* III · generation parameters */}
        <Section roman="III" name={t.generation.name} gloss={t.generation.gloss}>
          <Field name={t.generation.temperature} desc={t.generation.temperatureDesc}>
            <div className={styles.sliderRow}>
              <div className={styles.sliderTop}>
                <span>{t.generation.movement}</span>
                <span className={styles.reading}>{temp.toFixed(2)}</span>
              </div>
              <Slider
                value={temp}
                max={2}
                onChange={setTemp}
                onCommit={() => void save({ default_temperature: temp })}
              />
              <div className={styles.sliderBounds}>
                <span>{t.generation.boundLow}</span>
                <span>{t.generation.boundHigh}</span>
              </div>
            </div>
          </Field>

          <Field name={t.generation.topP} desc={t.generation.topPDesc}>
            <div className={styles.sliderRow}>
              <div className={styles.sliderTop}>
                <span>{t.generation.amplitude}</span>
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

          <Field name={t.generation.maxTokens} desc={t.generation.maxTokensDesc}>
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
              <span className={styles.tiSuffix}>{t.generation.tokens}</span>
            </div>
          </Field>

          <Field name={t.generation.seed} desc={t.generation.seedDesc}>
            <div className={styles.ti} style={{ maxWidth: 260 }}>
              <input
                className={`${styles.tiInput} ${styles.tiNum}`}
                type="text"
                inputMode="numeric"
                value={seed}
                placeholder={t.generation.seedPlaceholder}
                onChange={(e) => setSeed(e.target.value)}
                onBlur={commitSeed}
                onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              />
              <button
                className={styles.iconBtnInline}
                title={t.generation.resetToDefault}
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

        {/* IV · instrument */}
        <Section roman="IV" name={t.instrument.name} gloss={t.instrument.gloss}>
          <Field name={t.instrument.backend} desc={t.instrument.backendDesc}>
            <div className={styles.backendBox}>
              {backend ? (
                <span>
                  <span className={styles.backendName}>{backend}</span> · {t.backends[backend]}
                </span>
              ) : (
                <span>{t.instrument.examining}</span>
              )}
            </div>
            {detection && (
              <span className={styles.statLine}>
                {t.instrument.detected(
                  detection.system.cpu.physical_cores,
                  f.gb(detection.system.memory.total_bytes, 0),
                )}
              </span>
            )}
          </Field>

          <Field name={t.instrument.redo} desc={t.instrument.redoDesc}>
            <div className={styles.actionRow}>
              <button
                className={styles.gbtn}
                disabled={redetecting}
                onClick={() => void redetect()}
              >
                {redetecting ? t.instrument.examining : t.instrument.examineAgain}
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
                    ? t.instrument.cached(f.ago(detection.detected_at))
                    : t.instrument.examined(f.ago(detection.detected_at))}
                </span>
              )}
            </div>
          </Field>
        </Section>

        {/* V · about + irreversible actions */}
        <Section roman="V" name={t.about.name} gloss={t.about.gloss}>
          <Field name={t.about.about} desc={t.about.aboutDesc}>
            <div className={styles.about}>
              <p>
                <span className={styles.aboutKey}>{t.about.version}</span>
                <b>Abraxas {version ?? "—"}</b>
              </p>
              <p>
                <span className={styles.aboutKey}>{t.about.runtime}</span>
                llama.cpp{backend ? <> · {backend}</> : null}
              </p>
              <p>
                <span className={styles.aboutKey}>{t.about.license}</span>
                <em>{t.about.licenseValue}</em>
              </p>
              <p className={styles.aboutQuote}>{t.about.quote}</p>
            </div>
          </Field>

          <Field name={t.about.clearConversations} desc={t.about.clearConversationsDesc}>
            <DangerAction
              label={t.about.clearLabel}
              confirmLabel={t.about.clearConfirm}
              busyLabel={t.about.clearBusy}
              onConfirm={clearAllConversations}
            />
          </Field>

          <Field name={t.about.burn} desc={t.about.burnDesc}>
            <DangerAction
              label={t.about.burnLabel}
              confirmLabel={t.about.burnConfirm}
              busyLabel={t.about.burnBusy}
              onConfirm={burnEverything}
            />
          </Field>
        </Section>
      </div>
    </section>
  );
}
