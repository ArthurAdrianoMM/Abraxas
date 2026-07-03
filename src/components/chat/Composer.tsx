import { useCallback, useEffect, useRef } from "react";

interface ComposerProps {
  value: string;
  placeholder: string;
  generating: boolean;
  /** Chat is degraded (no model) — input stays visible but inert. */
  disabled: boolean;
  onChange: (value: string) => void;
  onSend: () => void;
  onStop: () => void;
}

export function Composer({
  value,
  placeholder,
  generating,
  disabled,
  onChange,
  onSend,
  onStop,
}: ComposerProps) {
  const taRef = useRef<HTMLTextAreaElement>(null);

  const autosize = useCallback(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(ta.scrollHeight, 160)}px`;
  }, []);

  useEffect(() => {
    autosize();
  }, [value, autosize]);

  useEffect(() => {
    taRef.current?.focus();
  }, []);

  const canSend = !disabled && !generating && value.trim().length > 0;

  return (
    <div className="composer-wrap">
      <div className="composer-inner">
        <div className="composer">
          <button className="icon-btn" title="anexar fragmento" aria-label="anexar">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path
                d="M10.5 4.5L5.7 9.3a2 2 0 1 0 2.8 2.8l5.5-5.5a3.5 3.5 0 0 0-5-5L3 7.6a5 5 0 1 0 7 7l4.5-4.5"
                stroke="currentColor"
                strokeWidth="1.2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
          <textarea
            ref={taRef}
            rows={1}
            value={value}
            placeholder={placeholder}
            autoComplete="off"
            spellCheck={false}
            disabled={disabled}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                if (generating) return;
                onSend();
              }
            }}
          />
          {generating ? (
            <button className="send-btn" aria-label="parar" title="parar" onClick={onStop}>
              <svg width="11" height="11" viewBox="0 0 16 16" fill="none">
                <rect
                  x="3.5"
                  y="3.5"
                  width="9"
                  height="9"
                  rx="1"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
              </svg>
            </button>
          ) : (
            <button
              className="send-btn"
              aria-label="enviar"
              onClick={onSend}
              disabled={!canSend}
              style={{ opacity: canSend ? 1 : 0.4, cursor: canSend ? "pointer" : "default" }}
            >
              <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
                <path
                  d="M8 13V3M8 3l-4 4M8 3l4 4"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
