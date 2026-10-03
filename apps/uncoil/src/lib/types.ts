// Mirrors of the serde types in uncoil-core. Keep in sync with crates/uncoil-core/src/{config,effect,layout}.rs.

/** `color::Rgb` is a tuple struct, so serde writes it as `[r, g, b]`. */
export type Rgb = [number, number, number];

export type WaveEffect = {
	kind: 'wave';
	/** 0 = sweeps left to right, 90 = back to front. */
	angle_deg: number;
	/** Seconds for one colour cycle to pass a point (higher = slower). */
	period_s: number;
	/** Width of one full rainbow, in key units. */
	wavelength: number;
	reverse: boolean;
};
export type SpectrumEffect = { kind: 'spectrum'; period_s: number };
export type StaticEffect = { kind: 'static'; color: Rgb };
export type OffEffect = { kind: 'off' };
export type Effect = WaveEffect | SpectrumEffect | StaticEffect | OffEffect;
export type EffectKind = Effect['kind'];

export interface DisplayPolicy {
	off_when_display_off: boolean;
	dim_level: number;
	fade_s: number;
}

export interface Placement {
	x: number;
	y: number;
}

export interface Config {
	effect: Effect;
	brightness: number;
	saturation: number;
	fps: number;
	display: DisplayPolicy;
	desk: Record<string, Placement>;
	openrgb_hardware_rainbow: boolean;
}

export interface Shape {
	name: string;
	row: number;
	col: number;
	x: number;
	y: number;
	w: number;
	h: number;
	is_key: boolean;
}

export type DeviceKind = 'keyboard' | 'mouse' | 'mousemat' | 'headset' | 'other';

export interface DeskDevice {
	id: string;
	name: string;
	kind: DeviceKind;
	x: number;
	y: number;
	w: number;
	h: number;
	shapes: Shape[];
}

export interface DeviceStatus {
	id: string;
	name: string;
	product_id: number;
	connection: string;
	fps: number;
	busy_retries: number;
	errors: number;
}

export interface Status {
	pid: number;
	version: string;
	started_unix: number;
	updated_unix: number;
	display: string;
	level: number;
	devices: DeviceStatus[];
	/** The daemon's own footprint: private memory, CPU as % of one core, executable size. */
	memory_bytes: number;
	cpu_percent: number;
	exe_bytes: number;
}

// ---- control pipe (uncoil_core::ipc). Keep in sync with crates/uncoil-core/src/{ipc,features/*}.rs ----

export type Feature = 'lighting' | 'hw_effects' | 'keymap' | 'profiles' | 'dial' | 'oled';
/** `hypershift` is the Fn layer. */
export type Layer = 'normal' | 'hypershift';

export interface DeviceInfo {
	id: string;
	name: string;
	kind: DeviceKind;
	product_id: number;
	connection: string;
	features: Feature[];
	/** Firmware effect spec showing instead of the software effect, e.g. `"wave left speed 40"`. */
	hw_effect: string | null;
}

export interface KeyInfo {
	id: number;
	name: string;
	/** Matches a keyboard shape name in the desk layout. */
	led: string | null;
}

export interface Capabilities {
	id: string;
	name: string;
	kind: DeviceKind;
	connected: boolean;
	features: Feature[];
	hw_effects: string[];
	keymap_layers: Layer[];
	keys: KeyInfo[];
	dial_modes: string[];
}

export interface KeyMapping {
	profile: number;
	key: number;
	name: string;
	layer: Layer;
	/** Spec string, e.g. `"key PRINT_SCREEN"`, `"razer 11"`, `"button 4"`, `"off"`. */
	function: string;
	description: string;
}

export interface WriteResult<T> {
	before: T;
	after: T;
	verified: boolean;
	unchanged: boolean;
}

export interface ProfileInfo {
	max: number;
	count: number;
	ids: number[];
	active: number | null;
}

export interface DialState {
	profile: number;
	mode_id: number;
	mode: string | null;
	display_order: number;
	enabled_functions: number;
}

export interface OledState {
	brightness: number | null;
	home_screen: string | null;
	home_screen_index: number | null;
	time_to_home: number | null;
	time_to_dim_minutes: number | null;
	language: number | null;
	active_item: string | null;
	low_battery_warning_percent: number | null;
	low_power_mode: boolean | null;
	animations_enabled: boolean[] | null;
	screensaver: number[] | null;
}

export interface EffectState {
	effect: string | null;
	storage: 'session' | 'onboard';
}
