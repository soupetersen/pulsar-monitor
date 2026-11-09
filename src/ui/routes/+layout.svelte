<script lang="ts">
  import "../app.css";
  import { locale } from "$lib/i18n";

  document.documentElement.lang = locale;
  // Windows draws Mica behind the window, macOS its vibrancy; Linux has no
  // system backdrop, so the card is drawn opaque there (see app.css).
  // Set by the Rust side: WebKitGTK's user agent claims to be Safari on a Mac.
  const ua = navigator.userAgent;
  document.documentElement.dataset.platform =
    (window as { __PULSAR_PLATFORM__?: string }).__PULSAR_PLATFORM__ ??
    (ua.includes("Windows") ? "windows" : ua.includes("Mac") ? "macos" : "linux");

  let { children } = $props();
</script>

{@render children()}
