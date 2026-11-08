import { locale } from "./i18n";

const nf1 = new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const nf2 = new Intl.NumberFormat(locale, { minimumFractionDigits: 2, maximumFractionDigits: 2 });

export const KB = 1024;
export const MB = 1024 ** 2;
export const GB = 1024 ** 3;

export const f1 = (n: number) => nf1.format(n);
export const f2 = (n: number) => nf2.format(n);
export const pct = (n: number) => `${Math.round(n)}%`;

export function bytes(b: number): string {
  if (b >= GB) return `${f1(b / GB)} GB`;
  if (b >= MB) return `${Math.round(b / MB)} MB`;
  return `${Math.round(b / KB)} KB`;
}

export function rate(b: number): string {
  if (b >= MB) return `${f1(b / MB)} MB/s`;
  return `${Math.round(b / KB)} KB/s`;
}

export function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return d > 0 ? `${d}d ${h}h` : `${h}h ${m}m`;
}

export function shortCpuName(name: string): string {
  return name.replace(/^AMD |^Intel\(R\) Core\(TM\) /, "").replace(/ \d+-Core Processor$/, "").replace(/ Processor$/, "");
}
