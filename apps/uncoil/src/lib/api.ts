// Typed wrappers over the Tauri commands in src-tauri/src/main.rs.
//
// Outside Tauri (plain `pnpm dev` in a browser, for design work) calls go to a local mock instead.
// The mock is a dynamic import, so it ships as a separate chunk the Tauri app never loads.
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { Config, DeskDevice, PreviewPress, Status } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (inTauri) return tauriInvoke<T>(cmd, args);
	const { mockInvoke } = await import('./mock');
	return mockInvoke<T>(cmd, args);
}

export const getConfig = () => invoke<Config>('get_config');
export const saveConfig = (config: Config) => invoke<void>('save_config', { config });
/** `connected`: ids of connected devices, so experimental devices with a layout join the desk. */
export const getDesk = (config: Config, connected: string[] = []) => invoke<DeskDevice[]>('get_desk', { config, connected });
/** Hex colour per shape per device, same order as `getDesk`, from the real effect engine. */
/** `presses` simulate key presses (reactive, ripple); `audio` simulates the audio level 0..1 (audio meter). */
export const previewFrame = (config: Config, t: number, presses: PreviewPress[] = [], audio = 0, connected: string[] = []) =>
	invoke<string[][]>('preview_frame', { config, t, presses, audio, connected });
/** `null` when the daemon is not running (no status file, or it is more than 10 s old). */
export const getStatus = () => invoke<Status | null>('get_status');

/** Open a web page in the system browser (the Tauri opener plugin; a new tab under `pnpm dev`). */
export async function openExternal(url: string) {
	if (inTauri) {
		const { openUrl } = await import('@tauri-apps/plugin-opener');
		await openUrl(url);
	} else window.open(url, '_blank', 'noopener');
}

/** A failed control-pipe call. `unreachable` means uncoild (0.2+, with the control pipe) isn't answering. */
export class DaemonError extends Error {
	constructor(
		message: string,
		readonly unreachable: boolean
	) {
		super(message);
	}
}

/** One command on uncoild's control pipe; the same commands as the `uncoil` CLI. */
export async function daemon<T>(cmd: string, device?: string | null, args?: Record<string, unknown>): Promise<T> {
	try {
		return await invoke<T>('daemon', { device: device ?? null, cmd, args: args ?? null });
	} catch (e) {
		const msg = String(e);
		throw new DaemonError(msg.replace(/^unreachable:\s*/, ''), msg.startsWith('unreachable:'));
	}
}
