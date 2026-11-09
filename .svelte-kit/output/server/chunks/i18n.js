// src/ui/lib/i18n.ts
var locale = typeof navigator !== "undefined" ? navigator.language || "en-US" : "en-US";
var t = locale.toLowerCase().startsWith("pt") ? {
	expand: "Expandir",
	minimizeToTray: "Minimizar para a bandeja",
	viewProcesses: {
		cpu: "Ver processos na CPU",
		gpu: "Ver processos na GPU",
		ram: "Ver processos na RAM"
	},
	system: "Sistema",
	uptime: (t) => `Ligado há ${t}`,
	toLight: "Mudar para tema claro",
	toDark: "Mudar para tema escuro",
	changeTheme: "Trocar tema",
	alwaysOnTop: "Sempre no topo",
	collapse: "Recolher",
	collapseAria: "Recolher para o modo compacto",
	showProcs: {
		cpu: "Ver processos na CPU",
		gpu: "Ver processos na GPU",
		ram: "Ver processos na RAM"
	},
	hideProcs: {
		cpu: "Ocultar processos na CPU",
		gpu: "Ocultar processos na GPU",
		ram: "Ocultar processos na RAM"
	},
	perThread: "Uso por thread",
	cpuTempOff: "Temperatura da CPU desativada. Clique para ativar (pede permissão de administrador uma vez).",
	cpuTempNeedsPawnio: "A temperatura da CPU precisa do driver gratuito PawnIO. Clique para abrir pawnio.eu.",
	cpuTempEnabling: "Ativando a temperatura da CPU…",
	enable: "Ativar",
	enabling: "Ativando…",
	updateAvailable: (v) => `Versão ${v} disponível. Clique para atualizar e reiniciar.`,
	updating: "Atualizando…",
	updateFailed: "A atualização falhou. Clique para tentar de novo.",
	temp: "Temp.",
	power: "Potência",
	fan: "Ventoinha",
	memory: "Memória",
	available: (gb) => `Disponível ${gb} GB`,
	network: "Rede",
	disk: "Disco",
	collecting: "Coletando dados…",
	loading: "Carregando…",
	noProcesses: "Nenhum processo usando agora.",
	processes: (n) => `${n} processos`,
	ofRam: (pct) => `${pct}% da RAM`
} : {
	expand: "Expand",
	minimizeToTray: "Minimize to tray",
	viewProcesses: {
		cpu: "View CPU processes",
		gpu: "View GPU processes",
		ram: "View RAM processes"
	},
	system: "System",
	uptime: (t) => `Up for ${t}`,
	toLight: "Switch to light theme",
	toDark: "Switch to dark theme",
	changeTheme: "Change theme",
	alwaysOnTop: "Always on top",
	collapse: "Collapse",
	collapseAria: "Collapse to compact mode",
	showProcs: {
		cpu: "Show CPU processes",
		gpu: "Show GPU processes",
		ram: "Show RAM processes"
	},
	hideProcs: {
		cpu: "Hide CPU processes",
		gpu: "Hide GPU processes",
		ram: "Hide RAM processes"
	},
	perThread: "Per-thread usage",
	cpuTempOff: "CPU temperature is off. Click to enable it (asks for administrator permission once).",
	cpuTempNeedsPawnio: "CPU temperature needs the free PawnIO driver. Click to open pawnio.eu.",
	cpuTempEnabling: "Enabling CPU temperature…",
	enable: "Enable",
	enabling: "Enabling…",
	updateAvailable: (v) => `Version ${v} is available. Click to update and restart.`,
	updating: "Updating…",
	updateFailed: "The update failed. Click to try again.",
	temp: "Temp.",
	power: "Power",
	fan: "Fan",
	memory: "Memory",
	available: (gb) => `${gb} GB available`,
	network: "Network",
	disk: "Disk",
	collecting: "Collecting data…",
	loading: "Loading…",
	noProcesses: "No processes using it right now.",
	processes: (n) => `${n} processes`,
	ofRam: (pct) => `${pct}% of RAM`
};
export { t as n, locale as t };
