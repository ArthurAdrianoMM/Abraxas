/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Dev-only: replays the first run without touching real data (`stores/ui.ts`). */
  readonly VITE_FORCE_ONBOARDING?: string;
  /** Recording build: hold on a curtain until a key starts the take (`lib/demo.ts`). */
  readonly VITE_DEMO_MODE?: string;
  /** Recording build: pin the UI language ("en" / "pt") instead of adopting the host's. */
  readonly VITE_DEMO_LOCALE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
