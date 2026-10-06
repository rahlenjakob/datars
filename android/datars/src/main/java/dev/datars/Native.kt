package dev.datars

/** JNI bindings to the datars runtime (crates/datars-ffi, `android` module). */
internal object Native {
    init { System.loadLibrary("datars_ffi") }
    external fun viewNew(allowScript: Boolean): Long
    external fun viewFree(v: Long)
    external fun loadDoc(v: Long, json: String): String?
    external fun openFile(v: Long, bytes: ByteArray): String?
    external fun openManifest(v: Long, bytes: ByteArray): String?
    external fun provideChunk(v: Long, hash: String, bytes: ByteArray): String?
    external fun resize(v: Long, w: Double, h: Double, dpr: Double)
    external fun frame(v: Long, now: Double): ByteArray?
    /** Draw frames with the GPU (Vulkan, else GLES; GLES first with [glFirst]) on [surface], `w × h`
     * px; false without a GPU. */
    external fun attachSurface(v: Long, surface: android.view.Surface, w: Int, h: Int, glFirst: Boolean): Boolean
    /** Stop drawing on the surface (before it's destroyed); frames are CPU pixels again. */
    external fun detachSurface(v: Long)
    /** Render a frame onto the attached surface; true while the next is due. */
    external fun drawFrame(v: Long, now: Double): Boolean
    /** The last frame as JSON: `engine` and `render` ms; on the GPU `draws`, `uploaded` (bytes of
     * new meshes), `rebuilt` (instances), `tessellated` (meshes), `standIns`. */
    external fun frameStats(v: Long): String?
    /** "Vulkan 4×MSAA" while frames go to the GPU, else null. */
    external fun gpuBackend(v: Long): String?
    external fun lastWidth(v: Long): Int
    external fun lastHeight(v: Long): Int
    /** Whether the last frame was mid-animation (render the next one). */
    external fun animating(v: Long): Boolean
    /** Tell the engine the time (seconds, frame clock) before an input, without rendering. */
    external fun setClock(v: Long, now: Double)
    /** Pointer input (`kind`: 0 move, 1 down, 2 up, 3 leave, 4 tap — a touch released where it went
     * down: clicks and inspects, with a finger's reach). The label it lands on, as a JSON string. */
    external fun pointer(v: Long, kind: Int, x: Double, y: Double): String?
    external fun event(v: Long, name: String): Boolean
    external fun setMode(v: Long, mode: Int)
    external fun status(v: Long): String?
    /** JSON `[{name, url} | {name, slot}]`: data the engine waits for. */
    external fun dataRequests(v: Long): String?
    external fun provideSource(v: Long, name: String, bytes: ByteArray): Boolean
    /** Bytes of a tiles archive from `offset` (answering a `range` data request). */
    external fun provideRange(v: Long, name: String, offset: Long, bytes: ByteArray): Boolean
    /** Run the click intent of the mark at `path` (a semantics item's path). */
    external fun activate(v: Long, path: String): Boolean
    external fun setSignalNum(v: Long, name: String, value: Double)
    /** A signal set to a JSON value (what a native picker chose); false when it doesn't parse. */
    external fun setSignalJson(v: Long, name: String, json: String): Boolean
    external fun gotoState(v: Long, index: Int): Boolean
    external fun seek(v: Long, pos: Double): Int
    external fun setPlaying(v: Long, on: Boolean)
    external fun setReducedMotion(v: Long, on: Boolean)
    /** The share (0.1–1) of the per-frame work budgets frames may spend: lower when they run long. */
    external fun setWorkScale(v: Long, scale: Double)
    /** Seconds (frame clock) when the next frame is due although nothing moves; NaN if none. */
    external fun wakeAt(v: Long): Double
    /** The last frame's pixel identity — the same on every platform (P1). */
    external fun pixelHash(v: Long): Long
}
