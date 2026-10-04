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
/** Fades in and out. No colours = cycles through the rainbow; one or two colours alternate. */
export type BreathingEffect = { kind: 'breathing'; colors: Rgb[]; period_s: number };
/** Random LEDs twinkle. No colours = random hues. `density` = share of LEDs lit at once (0..1). */
export type StarlightEffect = { kind: 'starlight'; colors: Rgb[]; density: number; twinkle_s: number };
/** Flames rising from the front edge of the desk. `height` 0..1 of the desk depth; `speed` 0.25..3. */
export type FireEffect = { kind: 'fire'; speed: number; height: number };
/** A rainbow turning around a centre point (desk units; null = the keyboard's centre). */
export type WheelEffect = { kind: 'wheel'; period_s: number; reverse: boolean; center: [number, number] | null };
/** Keys light up when pressed and fade. `color` null = a new rainbow hue per press. Needs key input. */
export type ReactiveEffect = { kind: 'reactive'; color: Rgb | null; fade_s: number };
/** A ring spreads across the desk from each pressed key. Needs key input. */
export type RippleEffect = { kind: 'ripple'; color: Rgb | null; speed: number; width: number; fade_s: number };
/** The desk fills left to right with the system audio level, green to yellow to red. */
export type AudioMeterEffect = { kind: 'audio_meter'; sensitivity: number };
/** A layered composition (Chroma Studio style), bottom layer first. */
export type StudioEffect = { kind: 'studio'; layers: StudioLayer[] };

/** Any effect that can be a layer (everything but a studio). */
export type LayerEffect =
	| WaveEffect
	| SpectrumEffect
	| StaticEffect
	| OffEffect
	| BreathingEffect
	| StarlightEffect
	| FireEffect
	| WheelEffect
	| ReactiveEffect
	| RippleEffect
	| AudioMeterEffect;
export type Effect = LayerEffect | StudioEffect;
export type EffectKind = Effect['kind'];

/** Which LEDs a layer covers. Shape names are the desk layout's (e.g. "W", "Left Shift", "Logo", "Edge"). */
export type Mask = { kind: 'all' } | { kind: 'devices'; ids: string[] } | { kind: 'keys'; device: string; shapes: string[] };

export interface StudioLayer {
	name: string;
	enabled: boolean;
	/** 0..1. Effects with their own transparency (reactive, ripple, starlight, audio meter) multiply this. */
	opacity: number;
	effect: LayerEffect;
	mask: Mask;
}

/** A simulated key press for the preview: desk position and time (seconds, same clock as `t`). */
export interface PreviewPress {
	x: number;
	y: number;
	t: number;
}

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
	/** What the OpenRGB hand-off sets: each device (part of its OpenRGB name) and the hardware mode to use. */
	openrgb?: { devices: { match: string; mode: string; ram?: boolean }[] };
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
	/** Razer devices (vendor 0x1532) connected with no device definition. Missing from older engines. */
	unknown_devices: UnknownDevice[];
}

export interface UnknownDevice {
	product_id: number;
	interfaces: number[];
}

// ---- control pipe (uncoil_core::ipc). Keep in sync with crates/uncoil-core/src/{ipc,features/*}.rs ----

export type Feature = 'lighting' | 'hw_effects' | 'keymap' | 'profiles' | 'dial' | 'oled' | 'dpi' | 'poll_rate' | 'power';
/** `experimental`: set up from OpenRazer/OpenRGB data, not yet confirmed on real hardware. */
export type Support = 'supported' | 'experimental';
/** A read-only check that gates onboard writes on experimental devices. Supported devices: `not_needed`. */
export type CheckState = 'passed' | 'failed' | 'untested' | 'not_needed';
export interface FeatureCheck {
	feature: Feature;
	state: CheckState;
	detail: string | null;
}
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
	support: Support;
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
	/** With `probe`: the lighting regions and firmware effects the device itself reports. */
	probed?: LightingProbe | null;
	support: Support;
	checks: FeatureCheck[];
	/** Features of a supported device not yet confirmed on it (their writes wait for a check). */
	unverified?: Feature[];
}

/** `capabilities` with `probe`: `0F/80` regions and, per region LED, the firmware effects it lists. */
export interface LightingProbe {
	regions: { led: number; rows: number; cols: number }[];
	effects: [number, string[]][];
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

export interface Dpi {
	x: number;
	y: number;
}

/** `performance.get`. Fields are null when the device doesn't support or couldn't read them. */
export interface PerformanceState {
	dpi: Dpi | null;
	dpi_min: number | null;
	dpi_max: number | null;
	/** `active` is 1-based. */
	stages: { active: number; list: Dpi[] } | null;
	/** 0 when stages aren't supported. */
	stages_max: number;
	poll_hz: number | null;
	/** [] when the polling rate can't be changed. */
	poll_rates: number[];
}

/** `power.get`. */
export interface PowerState {
	battery_pct: number | null;
	charging: boolean | null;
	idle_s: number | null;
	idle_range: [number, number] | null;
	low_battery_pct: number | null;
	low_battery_range: [number, number] | null;
}
