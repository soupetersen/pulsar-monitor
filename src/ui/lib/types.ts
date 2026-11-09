export type Panel = "none" | "cpu" | "gpu" | "ram";
export type Theme = "dark" | "light";

export interface Proc {
  name: string;
  count: number;
  /** % of total CPU */
  cpu: number;
  /** bytes */
  mem: number;
  /** % of the busiest GPU engine */
  gpu: number;
  /** dedicated VRAM bytes */
  vram: number;
}

export interface CpuTempStatus {
  supported: boolean;
  pawnio: boolean;
  enabled: boolean;
}

export interface ProcRow {
  name: string;
  sub: string;
  val: string;
  /** bar width, 0–100 */
  w: number;
}

export interface GpuStats {
  name: string;
  usage: number;
  tempC: number | null;
  powerW: number | null;
  powerLimitW: number | null;
  /** Power as % of the adapter maximum (non-NVIDIA GPUs). */
  powerPct: number | null;
  vramUsed: number | null;
  vramTotal: number | null;
  fanPct: number | null;
  /** Fan speed in RPM (non-NVIDIA GPUs). */
  fanRpm: number | null;
  clockMhz: number | null;
}

export interface Stats {
  host: string;
  uptimeSecs: number;
  cpu: {
    name: string;
    usage: number;
    clockGhz: number | null;
    tempC: number | null;
    powerW: number | null;
    threads: number;
    cores: number[];
  };
  ram: { used: number; total: number; available: number };
  gpu: GpuStats | null;
  net: { rxPerSec: number; txPerSec: number } | null;
  disk: { activePct: number; bytesPerSec: number } | null;
  procs: Proc[] | null;
  procsFor: Panel | null;
}
