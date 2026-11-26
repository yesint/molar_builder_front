/* tslint:disable */
/* eslint-disable */

export class Animation {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    cancel(): void;
    progress_json(): string;
    wait(): Promise<void>;
    readonly done: boolean;
}

/**
 * Stable handle to a molecule-owned PLY mesh.
 */
export class MeshHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    delete(): void;
    /**
     * JSON snapshot of name, visibility, material, style, sizes, smoothing, and counts.
     */
    details_json(): Promise<string>;
    /**
     * Export original PLY bytes. Display smoothing does not modify the source.
     */
    ply_bytes(): Promise<Uint8Array>;
    set line_width(value: number);
    set material(value: string);
    set point_size(value: number);
    set smoothing(value: number);
    set style(value: string);
    set visible(value: boolean);
}

/**
 * A loaded molecular group. All records remain in the scene; shared styles follow its shown member.
 */
export class MolGroup {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    current(): Promise<number>;
    members(): Promise<MolHandle[]>;
    reps(): Promise<RepHandle[]>;
    show(member: number): Promise<void>;
}

/**
 * Handle to a molecule in the viewer.
 */
export class MolHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add an interactions representation against a partner representation.
     */
    add_interactions(partner: RepHandle): RepHandle;
    /**
     * Import molecule-owned PLY bytes (positions are nanometers).
     */
    add_mesh_ply(name: string, bytes: Uint8Array): MeshHandle;
    /**
     * Add a representation. `sel` (a `Sel`, consumed) sets the selection;
     * `style`/`color`/`material` are names like `"cartoon"`/`"ss"`/`"Transparent"`.
     * Any argument may be omitted. Returns a handle.
     */
    add_rep(sel?: Sel | null, style?: string | null, color?: string | null, material?: string | null, width?: number | null): RepHandle;
    /**
     * Create a representation with its expression and appearance in one frame.
     */
    add_rep_text(text: string, style: string, color: string): Promise<RepHandle>;
    editing(): Promise<boolean>;
    /**
     * Export the edited ligand as mmCIF, preserving missing chemistry metadata.
     */
    export_cif(): Promise<string>;
    /**
     * Includes meshes created by the viewer UI or restored from a session.
     */
    meshes(): Promise<MeshHandle[]>;
    /**
     * This molecule's representations, as handles.
     */
    reps(): Promise<RepHandle[]>;
    /**
     * Enter or leave the native drawing editor for this molecule.
     */
    set_editing(enabled: boolean): Promise<void>;
    /**
     * Show or hide the native periodic box; remains editable in the viewer UI.
     */
    show_box(visible: boolean): Promise<void>;
}

export class Movie {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add any shared ViewStep, including simultaneous motion and saved-view transitions.
     */
    add_step_json(json: string): void;
    focus(mol: MolHandle, selection: string, duration: number): void;
    hold(duration: number): void;
    preview(): Promise<Animation>;
    rotate(yaw: number, pitch: number, duration: number, easing?: string | null): void;
    seek(time: number): Promise<void>;
    trajectory(mol: MolHandle, start: number, end: number, fps: number): void;
    unobstructed_view(rep: RepHandle, duration: number, zoom_out?: number | null, resolution?: number | null): void;
    zoom(factor: number, duration: number, easing?: string | null): void;
    readonly duration: number;
}

/**
 * Handle to a representation. Property setters (`rep.style = "lines"`, …) apply live.
 */
export class RepHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Read the current [fluid bonds, bond-color blend] values, including UI edits.
     */
    ballstick_bonds(): Promise<Float32Array>;
    /**
     * Remove this representation from the scene. The handle becomes stale.
     */
    remove(): Promise<void>;
    /**
     * Set the selection from a `Sel`'s atoms.
     */
    select(sel: Sel): void;
    /**
     * Apply a selection expression and preserve it in the representation's text field.
     * Rejects invalid syntax and stale representation handles.
     */
    select_text(text: string): Promise<void>;
    /**
     * Set fluid bonds and bond-color blending (both 0–1) for ball-and-stick.
     */
    set_ballstick_bonds(fluid_bonds: number, color_blend: number): Promise<void>;
    readonly molId: number;
    readonly repId: number;
    set color(value: string);
    set material(value: string);
    set style(value: string);
    set width(value: number);
}

/**
 * A frozen set of atom indices (the result of `System.select`). Consumed by
 * `add_rep`/`select` to set a representation's selection.
 */
export class Sel {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly numAtoms: number;
}

/**
 * All molecule records parsed once from a local file. No rendering or network
 * access is required to inspect counts or obtain an individual molecule.
 */
export class StructureFile {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    static from_bytes(name: string, bytes: Uint8Array): StructureFile;
    /**
     * Return a shared handle to a zero-based record. The file can be freed
     * without invalidating this handle or a viewer that uses it.
     */
    molecule(index: number): System;
    readonly numAtoms: number;
    readonly numMolecules: number;
}

/**
 * A structure parsed in the browser. Owns a molar `System` in an `Rc`; `add_mol`
 * shares it into the scene by reference (a cheap `Rc` clone), so the `System` stays
 * usable from JS (e.g. for more `select` calls) afterward.
 */
export class System {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Parse a structure from in-memory bytes (a fetched/uploaded `Uint8Array`). The
     * format is taken from `name`'s extension (pdb/gro/xyz/…). Throws on a read/parse
     * error.
     */
    static from_bytes(name: string, bytes: Uint8Array): System;
    /**
     * Evaluate a VMD-like selection string against this structure, returning a `Sel`
     * (its atom indices). Throws on a syntax error or an empty match.
     */
    select(text: string): Sel;
    readonly name: string;
    readonly numAtoms: number;
}

/**
 * A live handle to the running viewer. Returned by [`start`]; queues jobs to the
 * viewer, which applies them on the next frame. All methods apply live.
 */
export class Visualizer {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Load a molecule owned by the viewer and editable through its native UI.
     */
    add_editable(name: string, bytes: Uint8Array): Promise<MolHandle>;
    /**
     * Add every record of a parsed file as a normal molar_vis group.
     */
    add_file(file: StructureFile): Promise<MolGroup>;
    /**
     * Add a molecule, rendering **directly** from the parsed `System` (by reference);
     * returns a handle. Set `empty` to omit the default representation.
     */
    add_mol(sys: System, empty?: boolean | null): MolHandle;
    add_palette_color(color: Uint8Array): void;
    /**
     * Screen-space ambient occlusion: `strength` darkening, `radius` in nm.
     */
    ambient_occlusion(enabled: boolean, strength: number, radius: number): void;
    animate_json(json: string): Promise<Animation>;
    /**
     * Show/hide the orientation-axes gizmo, optionally setting its `corner`
     * (`"top_left"` / `"top_right"` / `"bottom_left"` / `"bottom_right"`).
     */
    axes(show: boolean, corner?: string | null): void;
    /**
     * Flat background color, RGB in 0–1.
     */
    background(r: number, g: number, b: number): void;
    /**
     * Vertical gradient background; `top`/`bottom` are `[r, g, b]` in 0–1.
     */
    background_gradient(top: Float32Array, bottom: Float32Array): void;
    /**
     * Read the current background, including solid/gradient mode and colors.
     */
    background_json(): Promise<string>;
    cancel_animation(): void;
    clear(): Promise<void>;
    /**
     * The current preset name, or Custom after manual edits.
     */
    color_preset(): Promise<string>;
    /**
     * JSON snapshot of current colors, including viewer UI/session edits.
     */
    colors_json(): Promise<string>;
    /**
     * Depth cueing (fog): `mode` is `"linear"`/`"exp"`/`"exp2"` (or `"none"`/`"off"` to
     * disable); `strength` = opacity at the back, `start` = where it begins (0–1).
     */
    depth_cue(enabled: boolean, mode: string, strength: number, start: number): void;
    focus(mol: MolHandle, sel: Sel, duration?: number | null, easing?: string | null): Promise<Animation>;
    go_to_view(json: string, duration?: number | null, easing?: string | null): Promise<Animation>;
    /**
     * The molecules currently in the scene, as handles.
     */
    mols(): Promise<MolHandle[]>;
    movie(): Movie;
    pan(dx: number, dy: number, duration?: number | null, easing?: string | null): Promise<Animation>;
    /**
     * Set the projection: `"perspective"` or `"orthographic"`.
     */
    projection(mode: string): void;
    remove_palette_color(index: number): void;
    reset_colors(): void;
    reset_element_color(number: number): void;
    reset_view(duration?: number | null, easing?: string | null): Promise<Animation>;
    roll(angle: number, duration?: number | null, easing?: string | null): Promise<Animation>;
    rotate(yaw: number, pitch: number, duration?: number | null, easing?: string | null): Promise<Animation>;
    /**
     * Set the camera orbit pivot (x, y, z) in nanometers.
     */
    rotation_center(x: number, y: number, z: number): void;
    /**
     * Persist current program settings, including color edits.
     */
    save_settings(): Promise<void>;
    /**
     * Apply atom, secondary-structure and gradient colors from a viewer preset.
     */
    set_color_preset(name: string): void;
    /**
     * `stops` contains three consecutive RGBA8 colors (12 bytes total).
     */
    set_continuous_gradient(stops: Uint8Array, midpoint: number): void;
    set_element_color(number: number, color: Uint8Array): void;
    /**
     * Replace the discrete palette with consecutive RGBA8 colors.
     */
    set_palette(colors: Uint8Array): void;
    set_palette_color(index: number, color: Uint8Array): void;
    set_secondary_color(index: number, color: Uint8Array): void;
    /**
     * Set the viewer UI theme without saving browser preferences.
     */
    set_theme(name: string): Promise<void>;
    /**
     * Apply sidebar/selection-toolbar visibility on the next viewer frame.
     */
    set_ui_visibility(selection_panel: boolean, selection_toolbar: boolean): Promise<void>;
    /**
     * Real-time cast shadows: `strength` scales how dark shadowed areas get.
     */
    shadows(enabled: boolean, strength: number): void;
    theme(): Promise<string>;
    /**
     * Read the actual UI state, including changes made through the viewer.
     */
    ui_visibility_json(): Promise<string>;
    unobstructed_view(rep: RepHandle, zoom_out: number, resolution: number, duration?: number | null, easing?: string | null): Promise<Animation>;
    unobstructed_view_multi(reps: Array<any>, zoom_out: number, resolution: number, duration?: number | null, easing?: string | null): Promise<Animation>;
    view_json(): Promise<string>;
    zoom(factor: number, duration?: number | null, easing?: string | null): Promise<Animation>;
}

/**
 * Boot the viewer onto an existing `<canvas id=canvas_id>` and return a handle to it.
 * Runs the eframe `WebRunner` on the page's animation-frame loop (single-threaded), so
 * it shares the page's thread; jobs sent through the returned [`Visualizer`] are
 * applied on the next frame. Throws if the canvas is missing or a viewer is already
 * running on this page (one viewer per page in v1).
 */
export function start(canvas_id: string): Visualizer;

/**
 * Start an embedded viewer with its selection sidebar and toolbar hidden.
 * Resolves after graphics initialization; rejects if the renderer cannot start.
 */
export function start_embedded(canvas_id: string): Promise<Visualizer>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_animation_free: (a: number, b: number) => void;
    readonly __wbg_meshhandle_free: (a: number, b: number) => void;
    readonly __wbg_molgroup_free: (a: number, b: number) => void;
    readonly __wbg_molhandle_free: (a: number, b: number) => void;
    readonly __wbg_movie_free: (a: number, b: number) => void;
    readonly __wbg_rephandle_free: (a: number, b: number) => void;
    readonly __wbg_sel_free: (a: number, b: number) => void;
    readonly __wbg_structurefile_free: (a: number, b: number) => void;
    readonly __wbg_system_free: (a: number, b: number) => void;
    readonly __wbg_visualizer_free: (a: number, b: number) => void;
    readonly animation_cancel: (a: number) => void;
    readonly animation_done: (a: number) => number;
    readonly animation_progress_json: (a: number) => [number, number, number, number];
    readonly animation_wait: (a: number) => any;
    readonly meshhandle_delete: (a: number) => void;
    readonly meshhandle_details_json: (a: number) => any;
    readonly meshhandle_ply_bytes: (a: number) => any;
    readonly meshhandle_set_line_width: (a: number, b: number) => [number, number];
    readonly meshhandle_set_material: (a: number, b: number, c: number) => void;
    readonly meshhandle_set_point_size: (a: number, b: number) => [number, number];
    readonly meshhandle_set_smoothing: (a: number, b: number) => [number, number];
    readonly meshhandle_set_style: (a: number, b: number, c: number) => void;
    readonly meshhandle_set_visible: (a: number, b: number) => void;
    readonly molgroup_current: (a: number) => any;
    readonly molgroup_members: (a: number) => any;
    readonly molgroup_reps: (a: number) => any;
    readonly molgroup_show: (a: number, b: number) => any;
    readonly molhandle_add_interactions: (a: number, b: number) => number;
    readonly molhandle_add_mesh_ply: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly molhandle_add_rep: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly molhandle_add_rep_text: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
    readonly molhandle_editing: (a: number) => any;
    readonly molhandle_export_cif: (a: number) => any;
    readonly molhandle_meshes: (a: number) => any;
    readonly molhandle_reps: (a: number) => any;
    readonly molhandle_set_editing: (a: number, b: number) => any;
    readonly molhandle_show_box: (a: number, b: number) => any;
    readonly movie_add_step_json: (a: number, b: number, c: number) => [number, number];
    readonly movie_duration: (a: number) => number;
    readonly movie_focus: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly movie_hold: (a: number, b: number) => [number, number];
    readonly movie_preview: (a: number) => any;
    readonly movie_rotate: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly movie_seek: (a: number, b: number) => any;
    readonly movie_trajectory: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly movie_unobstructed_view: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly movie_zoom: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly rephandle_ballstick_bonds: (a: number) => any;
    readonly rephandle_molId: (a: number) => number;
    readonly rephandle_remove: (a: number) => any;
    readonly rephandle_repId: (a: number) => number;
    readonly rephandle_select: (a: number, b: number) => void;
    readonly rephandle_select_text: (a: number, b: number, c: number) => any;
    readonly rephandle_set_ballstick_bonds: (a: number, b: number, c: number) => any;
    readonly rephandle_set_color: (a: number, b: number, c: number) => void;
    readonly rephandle_set_material: (a: number, b: number, c: number) => void;
    readonly rephandle_set_style: (a: number, b: number, c: number) => void;
    readonly rephandle_set_width: (a: number, b: number) => [number, number];
    readonly sel_numAtoms: (a: number) => number;
    readonly start: (a: number, b: number) => [number, number, number];
    readonly start_embedded: (a: number, b: number) => any;
    readonly structurefile_from_bytes: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly structurefile_molecule: (a: number, b: number) => [number, number, number];
    readonly structurefile_numAtoms: (a: number) => number;
    readonly structurefile_numMolecules: (a: number) => number;
    readonly system_from_bytes: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly system_name: (a: number) => [number, number];
    readonly system_numAtoms: (a: number) => number;
    readonly system_select: (a: number, b: number, c: number) => [number, number, number];
    readonly visualizer_add_editable: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly visualizer_add_file: (a: number, b: number) => any;
    readonly visualizer_add_mol: (a: number, b: number, c: number) => number;
    readonly visualizer_add_palette_color: (a: number, b: number, c: number) => [number, number];
    readonly visualizer_ambient_occlusion: (a: number, b: number, c: number, d: number) => void;
    readonly visualizer_animate_json: (a: number, b: number, c: number) => any;
    readonly visualizer_axes: (a: number, b: number, c: number, d: number) => [number, number];
    readonly visualizer_background: (a: number, b: number, c: number, d: number) => void;
    readonly visualizer_background_gradient: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly visualizer_background_json: (a: number) => any;
    readonly visualizer_cancel_animation: (a: number) => void;
    readonly visualizer_clear: (a: number) => any;
    readonly visualizer_color_preset: (a: number) => any;
    readonly visualizer_colors_json: (a: number) => any;
    readonly visualizer_depth_cue: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly visualizer_focus: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
    readonly visualizer_go_to_view: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
    readonly visualizer_mols: (a: number) => any;
    readonly visualizer_movie: (a: number) => number;
    readonly visualizer_pan: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
    readonly visualizer_projection: (a: number, b: number, c: number) => [number, number];
    readonly visualizer_remove_palette_color: (a: number, b: number) => void;
    readonly visualizer_reset_colors: (a: number) => void;
    readonly visualizer_reset_element_color: (a: number, b: number) => void;
    readonly visualizer_reset_view: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly visualizer_roll: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly visualizer_rotate: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
    readonly visualizer_rotation_center: (a: number, b: number, c: number, d: number) => [number, number];
    readonly visualizer_save_settings: (a: number) => any;
    readonly visualizer_set_color_preset: (a: number, b: number, c: number) => [number, number];
    readonly visualizer_set_continuous_gradient: (a: number, b: number, c: number, d: number) => [number, number];
    readonly visualizer_set_element_color: (a: number, b: number, c: number, d: number) => [number, number];
    readonly visualizer_set_palette: (a: number, b: number, c: number) => [number, number];
    readonly visualizer_set_palette_color: (a: number, b: number, c: number, d: number) => [number, number];
    readonly visualizer_set_secondary_color: (a: number, b: number, c: number, d: number) => [number, number];
    readonly visualizer_set_theme: (a: number, b: number, c: number) => any;
    readonly visualizer_set_ui_visibility: (a: number, b: number, c: number) => any;
    readonly visualizer_shadows: (a: number, b: number, c: number) => void;
    readonly visualizer_theme: (a: number) => any;
    readonly visualizer_ui_visibility_json: (a: number) => any;
    readonly visualizer_unobstructed_view: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
    readonly visualizer_unobstructed_view_multi: (a: number, b: any, c: number, d: number, e: number, f: number, g: number, h: number) => any;
    readonly visualizer_view_json: (a: number) => any;
    readonly visualizer_zoom: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke___wasm_bindgen_467f4f3b7e2e8c85___JsValue__core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_467f4f3b7e2e8c85___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke___js_sys_6dd63bfd807f0e0d___Function_fn_wasm_bindgen_467f4f3b7e2e8c85___JsValue_____wasm_bindgen_467f4f3b7e2e8c85___sys__Undefined___js_sys_6dd63bfd807f0e0d___Function_fn_wasm_bindgen_467f4f3b7e2e8c85___JsValue_____wasm_bindgen_467f4f3b7e2e8c85___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke___wasm_bindgen_467f4f3b7e2e8c85___JsValue______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke___js_sys_6dd63bfd807f0e0d___Array______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke___js_sys_6dd63bfd807f0e0d___Array______true__3: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke___core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_467f4f3b7e2e8c85___JsValue___true_: (a: number, b: number) => [number, number];
    readonly wasm_bindgen_467f4f3b7e2e8c85___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
