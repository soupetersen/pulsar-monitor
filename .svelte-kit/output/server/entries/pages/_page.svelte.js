import { a as ensure_array_like, b as attr, c as stringify, i as derived, n as attr_style, r as bind_props, s as spread_props, t as attr_class, x as escape_html } from "../../chunks/server.js";
import "../../chunks/index-server.js";
import { n as t, t as locale } from "../../chunks/i18n.js";
import { invoke } from "@tauri-apps/api/core";
import "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
// src/ui/lib/format.ts
var nf1 = new Intl.NumberFormat(locale, {
	minimumFractionDigits: 1,
	maximumFractionDigits: 1
});
var nf2 = new Intl.NumberFormat(locale, {
	minimumFractionDigits: 2,
	maximumFractionDigits: 2
});
var KB = 1024;
var MB = 1024 ** 2;
var GB = 1024 ** 3;
var f1 = (n) => nf1.format(n);
var f2 = (n) => nf2.format(n);
var pct = (n) => `${Math.round(n)}%`;
function bytes(b) {
	if (b >= GB) return `${f1(b / GB)} GB`;
	if (b >= MB) return `${Math.round(b / MB)} MB`;
	return `${Math.round(b / KB)} KB`;
}
function rate(b) {
	if (b >= MB) return `${f1(b / MB)} MB/s`;
	return `${Math.round(b / KB)} KB/s`;
}
function uptime(secs) {
	const d = Math.floor(secs / 86400);
	const h = Math.floor(secs % 86400 / 3600);
	const m = Math.floor(secs % 3600 / 60);
	return d > 0 ? `${d}d ${h}h` : `${h}h ${m}m`;
}
function shortCpuName(name) {
	return name.replace(/^AMD |^Intel\(R\) Core\(TM\) /, "").replace(/ \d+-Core Processor$/, "").replace(/ Processor$/, "");
}
// src/ui/lib/TempSetup.svelte
function TempSetup($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		/** Shown while CPU temperature is supported but not enabled yet. */
		let { status, busy, onclick, label = false } = $$props;
		let tip = derived(() => busy ? t.cpuTempEnabling : status.pawnio ? t.cpuTempOff : t.cpuTempNeedsPawnio);
		$$renderer.push(`<button${attr_class("temp-setup svelte-1s6lzn9", void 0, {
			"chip": label,
			"busy": busy
		})}${attr("disabled", busy, true)}${attr("title", tip())}${attr("aria-label", tip())}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="svelte-1s6lzn9"><path d="M14 4v10.54a4 4 0 1 1-4 0V4a2 2 0 0 1 4 0Z" class="svelte-1s6lzn9"></path></svg> <i class="dot svelte-1s6lzn9"></i> `);
		if (label) $$renderer.push(`<!--[0--><span class="svelte-1s6lzn9">${escape_html(busy ? t.enabling : t.enable)}</span>`);
		else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></button>`);
	});
}
// src/ui/lib/UpdateButton.svelte
function UpdateButton($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		/** Shown when a newer version is available; installs it and restarts. */
		let { version, busy, failed, onclick } = $$props;
		let tip = derived(() => busy ? t.updating : failed ? t.updateFailed : t.updateAvailable(version));
		$$renderer.push(`<button${attr_class("update svelte-m8qrv1", void 0, { "busy": busy })}${attr("disabled", busy, true)}${attr("title", tip())}${attr("aria-label", tip())}><svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="svelte-m8qrv1"><path d="M12 3v12" class="svelte-m8qrv1"></path><polyline points="7 10 12 15 17 10" class="svelte-m8qrv1"></polyline><path d="M5 21h14" class="svelte-m8qrv1"></path></svg> <i class="dot svelte-m8qrv1"></i></button>`);
	});
}
// src/ui/lib/Compact.svelte
function Compact($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { stats, onexpand, onhide, tempSetup, tempBusy, onenabletemp, update } = $$props;
		const C = 2 * Math.PI * 18;
		let rings = derived(() => {
			if (!stats) return [];
			const { cpu, ram, gpu } = stats;
			const cpuLabel = cpu.tempC != null ? `CPU · ${Math.round(cpu.tempC)}°C` : cpu.powerW != null ? `CPU · ${Math.round(cpu.powerW)} W` : "CPU";
			const list = [{
				panel: "cpu",
				value: cpu.usage,
				label: cpuLabel,
				title: t.viewProcesses.cpu
			}];
			if (gpu) list.push({
				panel: "gpu",
				value: gpu.usage,
				label: gpu.tempC != null ? `GPU · ${gpu.tempC}°C` : "GPU",
				title: t.viewProcesses.gpu
			});
			list.push({
				panel: "ram",
				value: ram.used / ram.total * 100,
				label: `RAM · ${f1(ram.used / GB)} GB`,
				title: t.viewProcesses.ram
			});
			if (gpu?.vramUsed != null && gpu.vramTotal) list.push({
				panel: "gpu",
				value: gpu.vramUsed / gpu.vramTotal * 100,
				label: `VRAM · ${f1(gpu.vramUsed / GB)} GB`,
				title: t.viewProcesses.gpu
			});
			return list;
		});
		$$renderer.push(`<div class="card svelte-1x8xgyb"><header data-tauri-drag-region="" class="svelte-1x8xgyb"><span class="host svelte-1x8xgyb"><i class="dot svelte-1x8xgyb"></i>${escape_html(stats?.host ?? "…")}</span> <div class="actions svelte-1x8xgyb">`);
		if (update) {
			$$renderer.push("<!--[0-->");
			UpdateButton($$renderer, spread_props([update]));
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> `);
		if (tempSetup) {
			$$renderer.push("<!--[0-->");
			TempSetup($$renderer, {
				status: tempSetup,
				busy: tempBusy,
				onclick: onenabletemp
			});
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> <button class="icon-btn"${attr("aria-label", t.expand)}${attr("title", t.expand)}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"></polyline><polyline points="9 21 3 21 3 15"></polyline><line x1="21" y1="3" x2="14" y2="10"></line><line x1="3" y1="21" x2="10" y2="14"></line></svg></button> <button class="icon-btn"${attr("aria-label", t.minimizeToTray)}${attr("title", t.minimizeToTray)}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12"></line></svg></button></div></header> `);
		if (!stats) {
			$$renderer.push(`<!--[0--><div class="rings svelte-1x8xgyb" aria-busy="true"${attr("aria-label", t.collecting)}${attr_style("", { "grid-template-columns": "repeat(4, minmax(0, 1fr))" })}><!--[-->`);
			const each_array = ensure_array_like([
				"CPU",
				"GPU",
				"RAM",
				"VRAM"
			]);
			for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
				let label = each_array[$$index];
				$$renderer.push(`<div class="ring placeholder svelte-1x8xgyb"><span class="dial svelte-1x8xgyb"><svg class="spinner svelte-1x8xgyb" aria-hidden="true" width="52" height="52" viewBox="0 0 44 44"><circle cx="22" cy="22" r="18" fill="none" stroke="var(--track)" stroke-width="4"></circle><circle cx="22" cy="22" r="18" fill="none" stroke="var(--accent)" stroke-width="4" stroke-linecap="round" stroke-dasharray="22 113.1"></circle></svg></span> <span class="label svelte-1x8xgyb">${escape_html(label)}</span></div>`);
			}
			$$renderer.push(`<!--]--></div>`);
		} else {
			$$renderer.push(`<!--[-1--><div class="rings svelte-1x8xgyb"${attr_style("", { "grid-template-columns": `repeat(${stringify(Math.max(rings().length, 1))}, minmax(0, 1fr))` })}><!--[-->`);
			const each_array_1 = ensure_array_like(rings());
			for (let i = 0, $$length = each_array_1.length; i < $$length; i++) {
				let r = each_array_1[i];
				$$renderer.push(`<button class="ring svelte-1x8xgyb"${attr("title", r.title)}${attr("aria-label", `${stringify(r.label)}: ${stringify(pct(r.value))}. ${stringify(r.title)}`)}><span class="dial svelte-1x8xgyb"><svg aria-hidden="true" width="52" height="52" viewBox="0 0 44 44"><circle cx="22" cy="22" r="18" fill="none" stroke="var(--track)" stroke-width="4"></circle><circle class="arc svelte-1x8xgyb" cx="22" cy="22" r="18" fill="none" stroke="var(--accent)" stroke-width="4" stroke-linecap="round" transform="rotate(-90 22 22)"${attr_style("", { "stroke-dasharray": `${stringify(Math.min(r.value, 100) / 100 * C)} 113.09733552923255` })}></circle></svg> <span class="value svelte-1x8xgyb">${escape_html(pct(r.value))}</span></span> <span class="label svelte-1x8xgyb">${escape_html(r.label)}</span></button>`);
			}
			$$renderer.push(`<!--]--></div>`);
		}
		$$renderer.push(`<!--]--></div>`);
	});
}
// src/ui/lib/ProcList.svelte
function ProcList($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { rows } = $$props;
		$$renderer.push(`<div class="list svelte-1jc9yem">`);
		if (rows === null) $$renderer.push(`<!--[0--><p class="empty muted svelte-1jc9yem">${escape_html(t.loading)}</p>`);
		else if (rows.length === 0) $$renderer.push(`<!--[1--><p class="empty muted svelte-1jc9yem">${escape_html(t.noProcesses)}</p>`);
		else {
			$$renderer.push(`<!--[-1--><!--[-->`);
			const each_array = ensure_array_like(rows);
			for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
				let r = each_array[$$index];
				$$renderer.push(`<div class="row svelte-1jc9yem"><div class="names svelte-1jc9yem"><span class="name svelte-1jc9yem">${escape_html(r.name)}</span> `);
				if (r.sub) $$renderer.push(`<!--[0--><span class="sub svelte-1jc9yem">${escape_html(r.sub)}</span>`);
				else $$renderer.push("<!--[-1-->");
				$$renderer.push(`<!--]--></div> <div class="bar svelte-1jc9yem"><div class="fill svelte-1jc9yem"${attr_style("", { width: `${stringify(r.w)}%` })}></div></div> <span class="val svelte-1jc9yem">${escape_html(r.val)}</span></div>`);
			}
			$$renderer.push(`<!--]-->`);
		}
		$$renderer.push(`<!--]--></div>`);
	});
}
// src/ui/lib/Sparkline.svelte
function Sparkline($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		/** Values 0–100, oldest first. Drawn right-aligned so the line grows from the right. */
		let { values, slots = 30 } = $$props;
		const W = 380;
		const H = 36;
		let points = derived(() => {
			const step = W / (slots - 1);
			const n = values.length;
			return values.map((v, i) => `${(W - (n - 1 - i) * step).toFixed(1)},${(34 - Math.min(v, 100) / 100 * 30).toFixed(1)}`).join(" ");
		});
		let area = derived(() => values.length > 1 ? `${points().split(" ")[0].split(",")[0]},${H} ${points()} ${W},${H}` : "");
		$$renderer.push(`<svg aria-hidden="true" viewBox="0 0 380 36" preserveAspectRatio="none" class="svelte-1li3d2x">`);
		if (values.length > 1) $$renderer.push(`<!--[0--><polygon${attr("points", area())} fill="var(--accent)" fill-opacity="0.12"></polygon><polyline${attr("points", points())} fill="none" stroke="var(--accent)" stroke-width="2" stroke-linejoin="round" vector-effect="non-scaling-stroke"></polyline>`);
		else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></svg>`);
	});
}
// src/ui/lib/Expanded.svelte
function Expanded($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { stats, cpuHist, gpuHist, panel = void 0, theme, onTop, oncollapse, ontheme, ontop, onhide, tempSetup, tempBusy, onenabletemp, update } = $$props;
		function rows(kind) {
			if (!stats || stats.procsFor !== kind || !stats.procs) return null;
			const total = stats.ram.total;
			const metric = (p) => kind === "cpu" ? p.cpu : kind === "gpu" ? Math.min(p.gpu, 100) : p.mem;
			const max = Math.max(...stats.procs.map(metric), 1e-6);
			return stats.procs.map((p) => {
				const many = p.count > 1 ? t.processes(p.count) : "";
				let sub;
				let val;
				if (kind === "cpu") {
					sub = [many, bytes(p.mem)];
					val = `${f1(p.cpu)}%`;
				} else if (kind === "gpu") {
					sub = [p.vram > 0 ? `${bytes(p.vram)} VRAM` : "", many];
					val = pct(Math.min(p.gpu, 100));
				} else {
					sub = [t.ofRam(f1(p.mem / total * 100)), many];
					val = bytes(p.mem);
				}
				return {
					name: p.name,
					sub: sub.filter(Boolean).join(" · "),
					val,
					w: Math.max(3, metric(p) / max * 100)
				};
			});
		}
		let ramPct = derived(() => stats ? stats.ram.used / stats.ram.total * 100 : 0);
		function procsToggle($$renderer, kind) {
			$$renderer.push(`<button class="toggle svelte-sdds99"${attr("aria-expanded", panel === kind)}><span>${escape_html(panel === kind ? t.hideProcs[kind] : t.showProcs[kind])}</span> <svg${attr_class("chev svelte-sdds99", void 0, { "open": panel === kind })} aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg></button> `);
			if (panel === kind) {
				$$renderer.push(`<!--[0--><div>`);
				ProcList($$renderer, { rows: rows(kind) });
				$$renderer.push(`<!----></div>`);
			} else $$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]-->`);
		}
		$$renderer.push(`<div class="card svelte-sdds99"><header data-tauri-drag-region="" class="svelte-sdds99"><div class="title svelte-sdds99"><span class="h svelte-sdds99">${escape_html(t.system)}</span> <span class="muted small svelte-sdds99">${escape_html(stats ? t.uptime(uptime(stats.uptimeSecs)) : "…")}</span></div> <div class="actions svelte-sdds99">`);
		if (update) {
			$$renderer.push("<!--[0-->");
			UpdateButton($$renderer, spread_props([update]));
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> <button class="icon-btn"${attr("aria-label", theme === "dark" ? t.toLight : t.toDark)}${attr("title", t.changeTheme)}>`);
		if (theme === "dark") $$renderer.push(`<!--[0--><svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"></circle><line x1="12" y1="2" x2="12" y2="4"></line><line x1="12" y1="20" x2="12" y2="22"></line><line x1="4.9" y1="4.9" x2="6.3" y2="6.3"></line><line x1="17.7" y1="17.7" x2="19.1" y2="19.1"></line><line x1="2" y1="12" x2="4" y2="12"></line><line x1="20" y1="12" x2="22" y2="12"></line><line x1="4.9" y1="19.1" x2="6.3" y2="17.7"></line><line x1="17.7" y1="6.3" x2="19.1" y2="4.9"></line></svg>`);
		else $$renderer.push(`<!--[-1--><svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"></path></svg>`);
		$$renderer.push(`<!--]--></button> <button${attr_class("icon-btn svelte-sdds99", void 0, { "active": onTop })}${attr("aria-pressed", onTop)}${attr("aria-label", t.alwaysOnTop)}${attr("title", t.alwaysOnTop)}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22"></line><path d="M5 17h14v-1.8a2 2 0 0 0-1.1-1.8l-1.8-.9A2 2 0 0 1 15 10.8V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.8a2 2 0 0 1-1.1 1.8l-1.8.9A2 2 0 0 0 5 15.2Z"></path></svg></button> <button class="icon-btn"${attr("aria-label", t.collapseAria)}${attr("title", t.collapse)}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 14 10 14 10 20"></polyline><polyline points="20 10 14 10 14 4"></polyline><line x1="14" y1="10" x2="21" y2="3"></line><line x1="3" y1="21" x2="10" y2="14"></line></svg></button> <button class="icon-btn"${attr("aria-label", t.minimizeToTray)}${attr("title", t.minimizeToTray)}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12"></line></svg></button></div></header> `);
		if (stats) {
			$$renderer.push("<!--[0-->");
			const c = stats.cpu;
			$$renderer.push(`<section class="svelte-sdds99"><div class="head svelte-sdds99"><span>CPU · ${escape_html(shortCpuName(c.name))}</span></div> <div class="big-row svelte-sdds99"><span class="big svelte-sdds99">${escape_html(pct(c.usage))}</span> `);
			Sparkline($$renderer, { values: cpuHist });
			$$renderer.push(`<!----></div> `);
			if (c.cores.length) {
				$$renderer.push(`<!--[0--><div class="threads svelte-sdds99"${attr("title", t.perThread)}><!--[-->`);
				const each_array = ensure_array_like(c.cores);
				for (let i = 0, $$length = each_array.length; i < $$length; i++) {
					let core = each_array[i];
					$$renderer.push(`<div class="thread svelte-sdds99"${attr_style("", { height: `${stringify(Math.max(8, core))}%` })}></div>`);
				}
				$$renderer.push(`<!--]--></div>`);
			} else $$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]--> <div class="stats svelte-sdds99"><div class="svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.temp)}</span> `);
			if (c.tempC != null) $$renderer.push(`<!--[0--><b class="svelte-sdds99">${escape_html(Math.round(c.tempC))}°C</b>`);
			else if (tempSetup) {
				$$renderer.push("<!--[1-->");
				TempSetup($$renderer, {
					status: tempSetup,
					busy: tempBusy,
					onclick: onenabletemp,
					label: true
				});
			} else $$renderer.push(`<!--[-1--><b class="svelte-sdds99">—</b>`);
			$$renderer.push(`<!--]--></div> <div class="svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.power)}</span><b class="svelte-sdds99">${escape_html(c.powerW != null ? `${Math.round(c.powerW)} W` : "—")}</b></div> <div class="svelte-sdds99"><span class="muted small svelte-sdds99">Clock</span><b class="svelte-sdds99">${escape_html(c.clockGhz ? `${f2(c.clockGhz)} GHz` : "—")}</b></div> <div class="svelte-sdds99"><span class="muted small svelte-sdds99">Threads</span><b class="svelte-sdds99">${escape_html(c.threads)}</b></div></div> `);
			procsToggle($$renderer, "cpu");
			$$renderer.push(`<!----></section> `);
			if (stats.gpu) {
				$$renderer.push("<!--[0-->");
				const g = stats.gpu;
				$$renderer.push(`<section class="svelte-sdds99"><div class="head svelte-sdds99"><span>GPU · ${escape_html(g.name)}</span> `);
				if (g.clockMhz != null) $$renderer.push(`<!--[0--><span>${escape_html(g.clockMhz)} MHz</span>`);
				else $$renderer.push("<!--[-1-->");
				$$renderer.push(`<!--]--></div> <div class="big-row svelte-sdds99"><span class="big svelte-sdds99">${escape_html(pct(g.usage))}</span> `);
				Sparkline($$renderer, { values: gpuHist });
				$$renderer.push(`<!----></div> <div class="stats svelte-sdds99"><div class="svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.temp)}</span><b class="svelte-sdds99">${escape_html(g.tempC != null ? `${g.tempC}°C` : "—")}</b></div> <div class="svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.power)}</span><b class="svelte-sdds99">${escape_html(g.powerW != null ? `${Math.round(g.powerW)} W` : g.powerPct != null ? `${Math.round(g.powerPct)}%` : "—")}</b></div> <div class="svelte-sdds99"><span class="muted small svelte-sdds99">VRAM</span><b class="svelte-sdds99">${escape_html(g.vramUsed != null && g.vramTotal ? `${f1(g.vramUsed / GB)} / ${g.vramTotal >= 10 * GB ? Math.round(g.vramTotal / GB) : f1(g.vramTotal / GB)} GB` : "—")}</b></div> <div class="svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.fan)}</span><b class="svelte-sdds99">${escape_html(g.fanPct != null ? `${g.fanPct}%` : g.fanRpm != null ? `${g.fanRpm} rpm` : "—")}</b></div></div> `);
				procsToggle($$renderer, "gpu");
				$$renderer.push(`<!----></section>`);
			} else $$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]--> <section class="svelte-sdds99"><div class="head svelte-sdds99"><span>${escape_html(t.memory)}</span> <span>${escape_html(f1(stats.ram.used / GB))} / ${escape_html(f1(stats.ram.total / GB))} GB</span></div> <div class="big-row svelte-sdds99"><span class="big svelte-sdds99">${escape_html(pct(ramPct()))}</span> <div class="ram-bar svelte-sdds99"><div class="bar svelte-sdds99"><div class="fill svelte-sdds99"${attr_style("", { width: `${stringify(ramPct())}%` })}></div></div> <span class="muted small svelte-sdds99">${escape_html(t.available(f1(stats.ram.available / GB)))}</span></div></div> `);
			procsToggle($$renderer, "ram");
			$$renderer.push(`<!----></section> <div class="pair svelte-sdds99"><div class="mini svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.network)}</span> <b class="svelte-sdds99">${escape_html(stats.net ? `↓ ${rate(stats.net.rxPerSec)} · ↑ ${rate(stats.net.txPerSec)}` : "—")}</b></div> <div class="mini svelte-sdds99"><span class="muted small svelte-sdds99">${escape_html(t.disk)}</span> <b class="svelte-sdds99">${escape_html(stats.disk ? `${pct(stats.disk.activePct)} · ${rate(stats.disk.bytesPerSec)}` : "—")}</b></div></div>`);
		} else $$renderer.push(`<!--[-1--><div class="loading svelte-sdds99" aria-busy="true"><svg class="spinner svelte-sdds99" aria-hidden="true" width="28" height="28" viewBox="0 0 44 44"><circle cx="22" cy="22" r="18" fill="none" stroke="var(--track)" stroke-width="4"></circle><circle cx="22" cy="22" r="18" fill="none" stroke="var(--accent)" stroke-width="4" stroke-linecap="round" stroke-dasharray="22 113.1"></circle></svg> <span class="muted">${escape_html(t.collecting)}</span></div>`);
		$$renderer.push(`<!--]--></div>`);
		bind_props($$props, { panel });
	});
}
// src/ui/routes/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		getCurrentWindow();
		let stats = null;
		let expanded = false;
		let panel = "none";
		let theme = loadTheme();
		let onTop = true;
		let cpuHist = [];
		let gpuHist = [];
		let maxHeight = 900;
		let tempStatus = null;
		let tempBusy = false;
		/** CPU temperature can be turned on but isn't yet: show the setup button. */
		let tempSetup = derived(() => tempStatus?.supported && !tempStatus.enabled ? tempStatus : null);
		let update = derived(() => null);
		async function enableTemp() {
			if (!tempStatus) return;
			if (!tempStatus.pawnio) {
				invoke("open_pawnio_site");
				return;
			}
			tempBusy = true;
			try {
				tempStatus = await invoke("set_cpu_temp", { enable: true });
			} finally {
				tempBusy = false;
			}
		}
		function loadTheme() {
			try {
				return localStorage.getItem("theme") === "light" ? "light" : "dark";
			} catch {
				return "dark";
			}
		}
		function expand(p) {
			if (p !== "none") panel = p;
			expanded = true;
		}
		const hide = () => invoke("hide_to_tray");
		let $$settled = true;
		let $$inner_renderer;
		function $$render_inner($$renderer) {
			$$renderer.push(`<div${attr_class("shell pulsar-shell svelte-oe9qwv", void 0, { "expanded": expanded })}${attr_style("", { "max-height": `${stringify(maxHeight)}px` })}>`);
			if (expanded) {
				$$renderer.push("<!--[0-->");
				Expanded($$renderer, {
					stats,
					cpuHist,
					gpuHist,
					theme,
					onTop,
					oncollapse: () => expanded = false,
					ontheme: () => theme = theme === "dark" ? "light" : "dark",
					ontop: () => invoke("set_on_top", { on: false }),
					onhide: hide,
					tempSetup: tempSetup(),
					tempBusy,
					onenabletemp: enableTemp,
					update: update(),
					get panel() {
						return panel;
					},
					set panel($$value) {
						panel = $$value;
						$$settled = false;
					}
				});
			} else {
				$$renderer.push("<!--[-1-->");
				Compact($$renderer, {
					stats,
					onexpand: expand,
					onhide: hide,
					tempSetup: tempSetup(),
					tempBusy,
					onenabletemp: enableTemp,
					update: update()
				});
			}
			$$renderer.push(`<!--]--></div>`);
		}
		do {
			$$settled = true;
			$$inner_renderer = $$renderer.copy();
			$$render_inner($$inner_renderer);
		} while (!$$settled);
		$$renderer.subsume($$inner_renderer);
	});
}
export { _page as default };
