// Typed wrappers over the Tauri commands in src-tauri/src/main.rs.
//
// Outside Tauri (plain `pnpm dev` in a browser, for design work) calls go to a local mock instead.
// The mock is a dynamic import, so it ships as a separate chunk the Tauri app never loads.
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { Config, DeskDevice, Status } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (inTauri) return tauriInvoke<T>(cmd, args);
	const { mockInvoke } = await import('./mock');
	return mockInvoke<T>(cmd, args);
}

export const getConfig = () => invoke<Config>('get_config');
export const saveConfig = (config: Config) => invoke<void>('save_config', { config });
export const getDesk = (config: Config) => invoke<DeskDevice[]>('get_desk', { config });
/** Hex colour per shape per device, same order as `getDesk`, from the real effect engine. */
export const previewFrame = (config: Config, t: number) =>
	invoke<string[][]>('preview_frame', { config, t });
/** `null` when the daemon is not running (no status file, or it is more than 10 s old). */
export const getStatus = () => invoke<Status | null>('get_status');
