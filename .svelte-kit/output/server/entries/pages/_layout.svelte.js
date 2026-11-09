import "../../chunks/server.js";
import { t as locale } from "../../chunks/i18n.js";
// src/ui/routes/+layout.svelte
function _layout($$renderer, $$props) {
	document.documentElement.lang = locale;
	const ua = navigator.userAgent;
	document.documentElement.dataset.platform = ua.includes("Windows") ? "windows" : ua.includes("Mac") ? "macos" : "linux";
	let { children } = $$props;
	children($$renderer);
	$$renderer.push(`<!---->`);
}
export { _layout as default };
