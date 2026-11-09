<script lang="ts">
  import "../app.css";
  import { locale } from "$lib/i18n";

  document.documentElement.lang = locale;
  // Windows draws Mica behind the window, macOS its vibrancy; Linux has no
  // system backdrop, so the card is drawn opaque there (see app.css).
  const ua = navigator.userAgent;
  document.documentElement.dataset.platform = ua.includes("Windows") ? "windows" : ua.includes("Mac") ? "macos" : "linux";

  let { children } = $props();
</script>

{@render children()}
