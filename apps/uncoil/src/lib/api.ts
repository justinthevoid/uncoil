// Typed wrappers over the Tauri commands in src-tauri/src/main.rs.
//
// Outside Tauri (plain `pnpm dev` in a browser, for design work) calls go to a local mock instead.
// The mock is a dynamic import, so it ships as a separate chunk the Tauri app never loads.
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { AppSettings, Config, DeskDevice, PreviewPress, Status, OpenRgbDevice } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (inTauri) return tauriInvoke<T>(cmd, args);
	const { mockInvoke } = await import('./mock');
	return mockInvoke<T>(cmd, args);
}

export const getConfig = () => invoke<Config>('get_config');
export const saveConfig = (config: Config) => invoke<void>('save_config', { config });
/** `connected`: ids of connected devices, so experimental devices with a layout join the desk. */
/** `external`: the devices OpenRGB reports (`status.openrgb.devices`), drawn as the PC column. */
export const getDesk = (config: Config, connected: string[] = [], external: OpenRgbDevice[] = []) =>
	invoke<DeskDevice[]>('get_desk', { config, connected, external });
/** Hex colour per shape per device, same order as `getDesk`, from the real effect engine. */
/** `presses` simulate key presses (reactive, ripple); `audio` simulates the audio level 0..1 (audio meter). */
export const previewFrame = (config: Config, t: number, presses: PreviewPress[] = [], audio = 0, connected: string[] = [], external: OpenRgbDevice[] = []) =>
	invoke<string[][]>('preview_frame', { config, t, presses, audio, connected, external });
/** `null` when the daemon is not running (no status file, or it is more than 10 s old). */
export const getStatus = () => invoke<Status | null>('get_status');

/** The app's own preferences (tray, start with Windows, battery notifications), kept apart from the engine's config. */
export const getAppSettings = () => invoke<AppSettings>('get_app_settings');
export const saveAppSettings = (settings: AppSettings) => invoke<void>('save_app_settings', { settings });

/** Called when something outside this window changed config.json (the tray's effect menu). Returns an unsubscribe. */
export async function onConfigChanged(handler: () => void): Promise<() => void> {
	if (!inTauri) return () => {};
	const { listen } = await import('@tauri-apps/api/event');
	return listen('config-changed', handler);
}

/** Open a web page in the system browser (the Tauri opener plugin; a new tab under `pnpm dev`). */
export async function openExternal(url: string) {
	if (inTauri) {
		const { openUrl } = await import('@tauri-apps/plugin-opener');
		await openUrl(url);
	} else window.open(url, '_blank', 'noopener');
}

/** Codes the engine puts on some errors (`ipc::codes`). */
export type ErrorCode = 'check_failed' | 'left_click_guard' | 'not_supported';

/** What the `daemon` command rejects with (src-tauri's `DaemonFailure`; the mock throws the same). */
export interface DaemonFailure {
	message: string;
	code: ErrorCode | null;
	unreachable: boolean;
}

/** A failed control-pipe call. `unreachable` means uncoild isn't answering on the pipe at all. */
export class DaemonError extends Error {
	constructor(
		message: string,
		readonly code: ErrorCode | null = null,
		readonly unreachable = false
	) {
		super(message);
	}
}

const isFailure = (e: unknown): e is DaemonFailure => typeof e === 'object' && e !== null && typeof (e as DaemonFailure).message === 'string';

/** One command on uncoild's control pipe; the same commands as the `uncoil` CLI. */
export async function daemon<T>(cmd: string, device?: string | null, args?: Record<string, unknown>): Promise<T> {
	try {
		return await invoke<T>('daemon', { device: device ?? null, cmd, args: args ?? null });
	} catch (e) {
		if (isFailure(e)) throw new DaemonError(e.message, e.code ?? null, e.unreachable === true);
		throw new DaemonError(String(e));
	}
}
