// C ABI of the datars runtime (crates/datars-ffi). Strings are UTF-8 pointer + length; JSON results
// are freed with datars_string_free.
#pragma once
#include <stdint.h>
#include <stddef.h>

typedef struct DatarsView DatarsView;

DatarsView *datars_view_new(int32_t allow_script);
void datars_view_free(DatarsView *v);
void datars_string_free(char *s);

char *datars_view_load_doc(DatarsView *v, const uint8_t *json, size_t len);
char *datars_view_open_file(DatarsView *v, const uint8_t *bytes, size_t len);
char *datars_view_open_manifest(DatarsView *v, const uint8_t *bytes, size_t len);
char *datars_view_provide_chunk(DatarsView *v, const uint8_t *hash, size_t hash_len, const uint8_t *bytes, size_t len);
int32_t datars_view_provide_source(DatarsView *v, const uint8_t *name, size_t name_len, const uint8_t *bytes, size_t len);
void datars_view_resize(DatarsView *v, double width, double height, double dpr);
int32_t datars_view_frame(DatarsView *v, double now);
const uint8_t *datars_view_pixels(DatarsView *v, uint32_t *width, uint32_t *height);
/* GPU frames (Metal) on the app's CAMetalLayer, width x height physical pixels: 1 when frames go to
 * the layer from now on (datars_view_frame presents), 0 without a GPU (frames stay CPU pixels). The
 * layer must outlive the view or datars_view_detach_gpu. */
int32_t datars_view_attach_metal_layer(DatarsView *v, void *layer, uint32_t width, uint32_t height);
void datars_view_detach_gpu(DatarsView *v);
/* "Metal 4xMSAA" while frames go to the GPU, else NULL; free with datars_string_free. */
char *datars_view_gpu_backend(DatarsView *v);
/* The last frame as JSON: engine and render ms; on the GPU draws, uploaded, rebuilt, tessellated, standIns. Free with datars_string_free. */
char *datars_view_frame_stats(DatarsView *v);
uint64_t datars_view_pixel_hash(DatarsView *v);
int32_t datars_view_goto(DatarsView *v, uint32_t index);
uint32_t datars_view_seek(DatarsView *v, double pos);
int32_t datars_view_wheel(DatarsView *v, double x, double y, double delta);
void datars_view_set_playing(DatarsView *v, int32_t on);
void datars_view_set_reduced_motion(DatarsView *v, int32_t on);
/* The reader's text size as a multiple of the theme's (Dynamic Type): labels scale and the chart lays out again. Clamped to 0.75-3. */
void datars_view_set_text_scale(DatarsView *v, double scale);
/* The reader's Bold Text setting: 1 draws every text heavier (regular as semibold, semibold and bold as bold). */
void datars_view_set_bold_text(DatarsView *v, int32_t on);
/* The share (0.1-1, default 1) of the per-frame work budgets frames may spend: lower it when frames run long. */
void datars_view_set_work_scale(DatarsView *v, double scale);
void datars_view_set_signal_num(DatarsView *v, const uint8_t *name, size_t len, double value);
/** Set a signal to a JSON value (a string, number, boolean, key set or null): what a native picker
 * chose for an engine-drawn select. Returns 0 when the JSON doesn't parse. */
int32_t datars_view_set_signal_json(DatarsView *v, const uint8_t *name, size_t len, const uint8_t *json, size_t json_len);
int32_t datars_view_activate(DatarsView *v, const uint8_t *path, size_t len);
/* A key on a brushable area or a pannable view (a readout item of kind "brush" or "pan", by its path): "Enter", "Escape", "ArrowLeft" … ("Shift+ArrowLeft"), "+", "-", "0". Returns 1 if the area used it. */
int32_t datars_view_key(DatarsView *v, const uint8_t *path, size_t len, const uint8_t *key, size_t key_len);
/* What the last tap, activation or key changed, in words (NULL when nothing to say), once: announce it politely. Free with datars_string_free. */
char *datars_view_take_announcement(DatarsView *v);
double datars_view_wake_at(DatarsView *v);
int32_t datars_view_animating(DatarsView *v);
void datars_view_set_clock(DatarsView *v, double now);
/* kind: 0 move, 1 down, 2 up, 3 leave, 4 tap (a touch released where it went down: clicks and inspects). Returns the label it lands on (JSON string) or NULL. */
char *datars_view_pointer(DatarsView *v, int32_t kind, double x, double y);
/* The cursor for where the pointer last was: 0 default, 1 pointer, 2 grab, 3 grabbing, 4 crosshair. */
int32_t datars_view_cursor(DatarsView *v);
int32_t datars_view_event(DatarsView *v, const uint8_t *name, size_t len);
void datars_view_set_mode(DatarsView *v, int32_t mode);
int32_t datars_view_set_tokens(DatarsView *v, const uint8_t *json, size_t len);
char *datars_view_status(DatarsView *v);
/* What a screen-reader bridge needs: the brief status and `readout` (the reading order: chart, headings, axes, data marks with values, buttons), as JSON. Free with datars_string_free. */
char *datars_view_accessibility(DatarsView *v);
char *datars_view_data_requests(DatarsView *v);
int32_t datars_view_provide_range(DatarsView *v, const uint8_t *name, size_t name_len, uint64_t offset, const uint8_t *bytes, size_t len);
/* Pictures the library doesn't decode (it carries no image decoder): a `decode` data request's
 * file, the app's decoded pixels for it (straight RGBA8, top row first), or why it couldn't. */
const uint8_t *datars_view_picture_file(DatarsView *v, const uint8_t *name, size_t name_len, size_t *len);
int32_t datars_view_provide_pixels(DatarsView *v, const uint8_t *name, size_t name_len, uint32_t width, uint32_t height, const uint8_t *rgba, size_t len);
void datars_view_picture_failed(DatarsView *v, const uint8_t *name, size_t name_len, const uint8_t *why, size_t why_len);
uint32_t datars_view_pictures_decoding(DatarsView *v);
