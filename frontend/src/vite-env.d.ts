/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_TURNSTILE_SITE_KEY: string;
}

interface Window {
  turnstile?: Turnstile.Turnstile;
  __nrpTurnstileOnLoad?: () => void;
}
