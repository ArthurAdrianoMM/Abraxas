import { useCallback, useEffect, useRef, useState } from "react";
import type { Conversation, ConversationGenerationParams } from "../../lib/tauri/bindings";
import { useConversationsStore } from "../../stores/conversations";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";
import styles from "./OrdersDrawer.module.css";

/** Last-resort fallbacks when settings haven't loaded yet — mirror
 *  `AppSettings::default()`. The live "herdar do padrão" hints come from
 *  the user's actual saved defaults. */
const FALLBACK = { temperature: 0.8, top_p: 0.95, max_completion_tokens: 512, seed: 1234 };

interface FieldState {
  temperature: number | null;
  top_p: number | null;
  max_completion_tokens: number | null;
  seed: number | null;
}

function fieldsFromConversation(c: Conversation): FieldState {
  return {
    temperature: c.temperature,
    top_p: c.top_p,
    max_completion_tokens: c.max_completion_tokens,
    seed: c.seed,
  };
}

function Slider({
  value,
  max,
  onChange,
}: {
  value: number;
  max: number;
  onChange: (v: number) => void;
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
    >
      <div className={styles.sliderFill} style={{ width: `${pct}%` }} />
      <div className={styles.sliderThumb} style={{ left: `${pct}%` }} />
    </div>
  );
}

function InheritToggle({
  inherit,
  hint,
  onToggle,
}: {
  inherit: boolean;
  hint: string;
  onToggle: () => void;
}) {
  return (
    <button
      className={`${styles.inherit} ${inherit ? styles.inheritOn : ""}`}
      onClick={onToggle}
    >
      <span className={styles.checkbox}></span>
      herdar do padrão ({hint})
    </button>
  );
}

function relativeAge(iso: string): string {
  const started = Date.parse(iso);
  if (Number.isNaN(started)) return "";
  const minutes = Math.max(0, Math.round((Date.now() - started) / 60_000));
  if (minutes < 1) return "iniciada agora";
  if (minutes < 60) return `iniciada há ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `iniciada há ${hours} h`;
  const days = Math.round(hours / 24);
  return `iniciada há ${days} d`;
}

export function OrdersDrawer({ conversation }: { conversation: Conversation }) {
  const setOrdersOpen = useUiStore((s) => s.setOrdersOpen);
  const updateParams = useConversationsStore((s) => s.updateParams);
  const messageCount = useConversationsStore((s) => s.messages.length);
  const settings = useSettingsStore((s) => s.settings);

  // "herdar do padrão" points at the user's saved defaults, not literals.
  const DEFAULTS = {
    temperature: settings?.default_temperature ?? FALLBACK.temperature,
    top_p: settings?.default_top_p ?? FALLBACK.top_p,
    max_completion_tokens: settings?.default_max_completion_tokens ?? FALLBACK.max_completion_tokens,
    seed: settings?.default_seed ?? FALLBACK.seed,
  };

  const [fields, setFields] = useState<FieldState>(() => fieldsFromConversation(conversation));
  const [saving, setSaving] = useState(false);

  // Re-seed local state when the drawer is opened for another conversation.
  useEffect(() => {
    setFields(fieldsFromConversation(conversation));
  }, [conversation.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const close = () => setOrdersOpen(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const apply = async () => {
    setSaving(true);
    try {
      // Fields the drawer doesn't expose (top_k, repeat_*) pass through
      // untouched so applying never clobbers them.
      const params: ConversationGenerationParams = {
        temperature: fields.temperature,
        top_p: fields.top_p,
        top_k: conversation.top_k,
        repeat_penalty: conversation.repeat_penalty,
        repeat_last_n: conversation.repeat_last_n,
        seed: fields.seed,
        max_completion_tokens: fields.max_completion_tokens,
      };
      await updateParams(conversation.id, params);
      close();
    } finally {
      setSaving(false);
    }
  };

  const turnos = messageCount === 1 ? "1 turno" : `${messageCount} turnos`;

  return (
    <>
      <div className={styles.scrim} onClick={close} />
      <aside
        className={styles.drawer}
        role="dialog"
        aria-label="ordens desta conversa"
        aria-modal="true"
      >
        <div className={styles.head}>
          <div className={styles.kicker}>
            <span>
              <span className={styles.step}>ordens</span> · desta conversa
            </span>
            <button className={styles.close} onClick={close} aria-label="fechar">
              <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
                <path
                  d="M4 4l8 8M12 4l-8 8"
                  stroke="currentColor"
                  strokeWidth="1.4"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </div>
          <h1 className={styles.heading}>As ordens desta conversa.</h1>
          <span className={styles.convName}>
            <b>“{conversation.title}”</b> · {turnos} · {relativeAge(conversation.created_at)}
          </span>
        </div>

        <div className={styles.body}>
          <div className={styles.section}>
            <div className={styles.sectionHead}>
              <span className={styles.sectionName}>i · a medida do verbo</span>
              <p className={styles.sectionGloss}>
                parâmetros desta conversa. desmarque para herdar das preferências.
              </p>
            </div>

            {/* temperatura */}
            <div
              className={`${styles.field} ${fields.temperature === null ? styles.fieldMuted : ""}`}
            >
              <div className={styles.fieldTop}>
                <span className={styles.label}>temperatura</span>
                <InheritToggle
                  inherit={fields.temperature === null}
                  hint={DEFAULTS.temperature.toFixed(2)}
                  onToggle={() =>
                    setFields((f) => ({
                      ...f,
                      temperature: f.temperature === null ? DEFAULTS.temperature : null,
                    }))
                  }
                />
              </div>
              <div className={styles.valueArea}>
                <div className={styles.sliderRow}>
                  <div className={styles.sliderTop}>
                    <span>movimento</span>
                    <span className={styles.reading}>
                      {(fields.temperature ?? DEFAULTS.temperature).toFixed(2)}
                      {fields.temperature !== null && (
                        <span className={styles.readingVs}>
                          · padrão {DEFAULTS.temperature.toFixed(2)}
                        </span>
                      )}
                    </span>
                  </div>
                  <Slider
                    value={fields.temperature ?? DEFAULTS.temperature}
                    max={2}
                    onChange={(v) => setFields((f) => ({ ...f, temperature: v }))}
                  />
                </div>
              </div>
            </div>

            {/* top-p */}
            <div className={`${styles.field} ${fields.top_p === null ? styles.fieldMuted : ""}`}>
              <div className={styles.fieldTop}>
                <span className={styles.label}>top-p</span>
                <InheritToggle
                  inherit={fields.top_p === null}
                  hint={DEFAULTS.top_p.toFixed(2)}
                  onToggle={() =>
                    setFields((f) => ({
                      ...f,
                      top_p: f.top_p === null ? DEFAULTS.top_p : null,
                    }))
                  }
                />
              </div>
              <div className={styles.valueArea}>
                <div className={styles.sliderRow}>
                  <div className={styles.sliderTop}>
                    <span>amplitude</span>
                    <span className={styles.reading}>
                      {(fields.top_p ?? DEFAULTS.top_p).toFixed(2)}
                      {fields.top_p !== null && (
                        <span className={styles.readingVs}>
                          · padrão {DEFAULTS.top_p.toFixed(2)}
                        </span>
                      )}
                    </span>
                  </div>
                  <Slider
                    value={fields.top_p ?? DEFAULTS.top_p}
                    max={1}
                    onChange={(v) => setFields((f) => ({ ...f, top_p: v }))}
                  />
                </div>
              </div>
            </div>

            {/* máximo de tokens */}
            <div
              className={`${styles.field} ${
                fields.max_completion_tokens === null ? styles.fieldMuted : ""
              }`}
            >
              <div className={styles.fieldTop}>
                <span className={styles.label}>máximo de tokens</span>
                <InheritToggle
                  inherit={fields.max_completion_tokens === null}
                  hint={String(DEFAULTS.max_completion_tokens)}
                  onToggle={() =>
                    setFields((f) => ({
                      ...f,
                      max_completion_tokens:
                        f.max_completion_tokens === null
                          ? DEFAULTS.max_completion_tokens
                          : null,
                    }))
                  }
                />
              </div>
              <div className={styles.valueArea}>
                <div className={styles.numBox}>
                  <input
                    type="text"
                    inputMode="numeric"
                    value={fields.max_completion_tokens ?? ""}
                    placeholder={String(DEFAULTS.max_completion_tokens)}
                    onChange={(e) => {
                      const n = parseInt(e.target.value, 10);
                      setFields((f) => ({
                        ...f,
                        max_completion_tokens: Number.isFinite(n) && n > 0 ? n : null,
                      }));
                    }}
                  />
                  <span className={styles.numUnit}>tokens</span>
                </div>
              </div>
            </div>

            {/* semente */}
            <div className={`${styles.field} ${fields.seed === null ? styles.fieldMuted : ""}`}>
              <div className={styles.fieldTop}>
                <span className={styles.label}>semente</span>
                <InheritToggle
                  inherit={fields.seed === null}
                  hint={
                    settings?.default_seed != null ? String(settings.default_seed) : "aleatória"
                  }
                  onToggle={() =>
                    setFields((f) => ({
                      ...f,
                      seed: f.seed === null ? DEFAULTS.seed : null,
                    }))
                  }
                />
              </div>
              <div className={styles.valueArea}>
                <div className={styles.numBox}>
                  <input
                    type="text"
                    inputMode="numeric"
                    value={fields.seed ?? ""}
                    placeholder={`padrão · ex.: 365`}
                    onChange={(e) => {
                      const n = parseInt(e.target.value, 10);
                      setFields((f) => ({
                        ...f,
                        seed: Number.isFinite(n) && n >= 0 ? n : null,
                      }));
                    }}
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className={styles.foot}>
          <button
            className={styles.verbLink}
            onClick={() =>
              setFields({
                temperature: null,
                top_p: null,
                max_completion_tokens: null,
                seed: null,
              })
            }
          >
            restaurar tudo ao padrão
          </button>
          <button className={styles.applyBtn} onClick={apply} disabled={saving}>
            aplicar à conversa
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
        </div>
      </aside>
    </>
  );
}
