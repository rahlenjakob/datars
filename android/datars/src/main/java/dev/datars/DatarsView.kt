package dev.datars

import android.content.Context
import android.graphics.Bitmap
import android.graphics.SurfaceTexture
import android.os.SystemClock
import android.util.AttributeSet
import android.util.Log
import android.view.Choreographer
import android.view.MotionEvent
import android.view.Surface
import android.view.TextureView
import android.view.View
import org.json.JSONObject
import java.net.URL
import java.nio.ByteBuffer

/**
 * Shows a datars chart. `load("https://charts.example.com/c/votes")` fetches the bundle (manifest +
 * the chunks the runtime asks for); frames run only while something moves. Frames go to the GPU
 * (Vulkan, else GLES) on the view's surface; without a GPU the CPU reference draws them.
 */
class DatarsView @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : TextureView(context, attrs), TextureView.SurfaceTextureListener {
    private val handle = Native.viewNew(true).also {
        // "Remove animations" sets the animator scale to 0: honour it as reduced motion.
        val scale = android.provider.Settings.Global.getFloat(context.contentResolver, android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
        Native.setReducedMotion(it, scale == 0f)
        Native.setTextScale(it, context.resources.configuration.fontScale.toDouble())
        Native.setWorkScale(it, Work.share)
    }

    /** The chart's text size as a multiple of the theme's: `null` (the default) follows the
     * reader's font size (Settings › Display › Font size, `Configuration.fontScale`), also when an
     * activity that handles `fontScale` itself stays up through a change; a number fixes it. */
    var textScale: Double? = null
        set(v) { field = v; applyTextSize() }

    private fun applyTextSize() {
        Native.setTextScale(handle, textScale ?: resources.configuration.fontScale.toDouble())
        requestFrame()
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        applyTextSize()
        applyBoldText()
    }

    /** Draw the chart's text heavier: `null` (the default) follows the reader's Bold text setting
     * (Settings › Display, `Configuration.fontWeightAdjustment`, Android 12+), live; a value fixes it. */
    var boldText: Boolean? = null
        set(v) { field = v; applyBoldText() }

    private fun applyBoldText() {
        val system = android.os.Build.VERSION.SDK_INT >= 31 && resources.configuration.fontWeightAdjustment > 0
        Native.setBoldText(handle, boldText ?: system)
        requestFrame()
    }

    /** Whether the reader's contrast setting raises the chart to its high-contrast mode (default):
     * Settings › Accessibility › Contrast at medium or high (Android 14+, `UiModeManager.contrast`)
     * or High contrast text (where the system lets apps read it). Live. */
    var followsContrast = true
        set(v) { field = v; applyMode() }
    private var dark = false

    /** The reader asks for more contrast. */
    private fun wantsContrast(): Boolean {
        if (android.os.Build.VERSION.SDK_INT >= 34) {
            val ui = context.getSystemService(android.app.UiModeManager::class.java)
            if (ui != null && ui.contrast >= 0.5f) return true
        }
        return runCatching { android.provider.Settings.Secure.getInt(context.contentResolver, "high_text_contrast_enabled", 0) == 1 }.getOrDefault(false)
    }

    /** The mode last given to the engine (-1: none yet; the engine starts light). */
    private var modeTold = -1

    private fun applyMode() {
        val m = if (followsContrast && wantsContrast()) 2 else if (dark) 1 else 0
        if (m == modeTold || (modeTold < 0 && m == 0)) return
        modeTold = m
        Native.setMode(handle, m)
        requestFrame()
    }

    private val contrastObserver = object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) { applyMode() }
    }
    private var contrastListener: Any? = null

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        runCatching { context.contentResolver.registerContentObserver(android.provider.Settings.Secure.getUriFor("high_text_contrast_enabled"), false, contrastObserver) }
        if (android.os.Build.VERSION.SDK_INT >= 34) {
            val ui = context.getSystemService(android.app.UiModeManager::class.java)
            val l = android.app.UiModeManager.ContrastChangeListener { applyMode() }
            ui?.addContrastChangeListener(context.mainExecutor, l)
            contrastListener = l
        }
        applyMode()
        applyBoldText()
    }
    /** The work share this view's engine was last given (see [Work]). */
    private var workTold = Work.share
    private val wake = Runnable { requestFrame() }
    /** Where relative data URLs resolve (the bundle's address). */
    private var dataBase: String? = null
    private val fetching = mutableSetOf<String>()

    /** The engine is used from the main thread only; network work runs on a background thread. */
    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private fun io(block: () -> Unit) = Thread(block, "datars-io").start()
    private fun get(url: java.net.URL): ByteArray? = runCatching { url.openConnection().apply { useCaches = false }.getInputStream().use { it.readBytes() } }.getOrNull()

    /** Decode the pictures the engine holds (it carries no decoder) with the platform's —
     * PNG, JPEG, WebP, HEIF: whatever BitmapFactory reads — and hand over their pixels. */
    private fun decodePictures() {
        if (Native.picturesDecoding(handle) == 0) return
        val reqs = org.json.JSONArray(Native.dataRequests(handle) ?: "[]")
        for (i in 0 until reqs.length()) {
            val r = reqs.getJSONObject(i)
            if (!r.optBoolean("decode")) continue
            val name = r.optString("name")
            val file = Native.pictureFile(handle, name) ?: continue
            val opts = android.graphics.BitmapFactory.Options().apply {
                inPremultiplied = false
                inPreferredConfig = Bitmap.Config.ARGB_8888
                inScaled = false
            }
            val bmp = android.graphics.BitmapFactory.decodeByteArray(file, 0, file.size, opts)
            if (bmp == null) {
                Native.pictureFailed(handle, name, "the platform's decoder couldn't read the picture")
                continue
            }
            // ARGB_8888 lies in memory as R, G, B, A bytes: straight RGBA8, as the engine takes it.
            val px = ByteBuffer.allocate(bmp.byteCount)
            bmp.copyPixelsToBuffer(px)
            Native.providePixels(handle, name, bmp.width, bmp.height, px.array())
            bmp.recycle()
        }
    }

    /** Fetch what the engine asks for (URL sources: first loads, live refreshes) and hand it over. */
    private fun fetchData() {
        val reqs = org.json.JSONArray(Native.dataRequests(handle) ?: "[]")
        for (i in 0 until reqs.length()) {
            val r = reqs.getJSONObject(i)
            val name = r.optString("name"); val rel = r.optString("url")
            val range = r.optJSONArray("range")
            val id = if (range != null) "$name@${range.getLong(0)}" else name
            if (rel.isEmpty() || !fetching.add(id)) continue
            val url = java.net.URL(java.net.URL(dataBase ?: rel), rel)
            io {
                if (range != null) {
                    // Tile archives come in byte ranges (HTTP Range); a server that ignores Range
                    // (or a file: URL) sends the whole archive, which is sliced here.
                    val off = range.getLong(0); val len = range.getLong(1)
                    val bytes = runCatching {
                        val c = url.openConnection()
                        c.setRequestProperty("Range", "bytes=$off-${off + len - 1}")
                        c.useCaches = false
                        val all = c.getInputStream().use { it.readBytes() }
                        if (all.size.toLong() != len && all.size >= off + len) all.copyOfRange(off.toInt(), (off + len).toInt()) else all
                    }.getOrNull()
                    main.post { fetching.remove(id); if (bytes != null) { Native.provideRange(handle, name, off, bytes); requestFrame() } }
                } else {
                    val bytes = get(url)
                    main.post { fetching.remove(id); if (bytes != null) { Native.provideSource(handle, name, bytes); requestFrame() } }
                }
            }
        }
    }

    private var bitmap: Bitmap? = null
    /** The surface frames go to (the TextureView's), and whether the GPU draws on it. */
    private var surface: Surface? = null
    private var gpu = false

    init {
        isOpaque = false
        surfaceTextureListener = this
    }

    override fun onSurfaceTextureAvailable(st: SurfaceTexture, w: Int, h: Int) {
        val s = Surface(st).also { surface = it }
        applySize()
        gpu = Native.attachSurface(handle, s, w, h, emulator)
        Log.i("datars", "frames: ${Native.gpuBackend(handle) ?: "CPU pixels (no GPU for the surface)"}")
        requestFrame()
    }

    override fun onSurfaceTextureSizeChanged(st: SurfaceTexture, w: Int, h: Int) { applySize(); requestFrame() }

    override fun onSurfaceTextureDestroyed(st: SurfaceTexture): Boolean {
        // The renderer lets go of the surface before it's released.
        Native.detachSurface(handle)
        gpu = false
        surface?.release()
        surface = null
        return true
    }

    override fun onSurfaceTextureUpdated(st: SurfaceTexture) {}

    /** An Android emulator: its Vulkan (gfxstream) aborts the process allocating memory on macOS
     * hosts, which no one can catch, so emulators draw with GLES. */
    private val emulator = android.os.Build.HARDWARE.let { it.contains("ranchu") || it.contains("goldfish") } || android.os.Build.PRODUCT.contains("sdk_gphone")

    private val start = SystemClock.uptimeMillis()
    /** The frame clock, seconds. */
    private fun now() = (SystemClock.uptimeMillis() - start) / 1000.0

    /** Tell the engine the time before an input: frames stop while nothing moves, and a transition
     * started at the last frame's time would count as long started (a flight would jump). */
    private fun <T> input(block: () -> T): T { Native.setClock(handle, now()); return block() }

    private var frameQueued = false
    /** The vsync the frame being drawn belongs to (ns), from Choreographer. */
    private var vsyncNanos = 0L
    private val tick = Choreographer.FrameCallback { t -> frameQueued = false; vsyncNanos = t; renderFrame() }

    /** Every frame, for a benchmark or a stats readout: the CPU time it took (the engine, and the
     * GPU encode and present; ms), whether something moved, and its vsync (ns). */
    var onFrame: ((cpuMs: Double, moving: Boolean, vsyncNanos: Long) -> Unit)? = null
    /** Called once a chart has loaded (and its first frame is drawn). */
    var onLoad: (() -> Unit)? = null

    /** Step through every state of the chart, `holdMs` apart, timing each transition frame by
     * frame — the web runtime's frame profiler's summary: frames, fps, the worst gap between frames,
     * 60 Hz frames dropped, CPU per frame (p50, p95) and its engine and render parts, and the GPU
     * work behind them (draws, bytes uploaded, instances rebuilt, meshes tessellated). `done` gets
     * one per transition. For measuring a chart on a device (the sample app's `bench` extra). */
    fun runBenchmark(holdMs: Long = 2500, done: (List<JSONObject>) -> Unit) {
        val states = JSONObject(Native.status(handle) ?: "{}").optJSONArray("states") ?: org.json.JSONArray()
        if (states.length() < 2) { done(emptyList()); return }
        val results = mutableListOf<JSONObject>()
        val frames = mutableListOf<Frame>()
        var recording = false
        val before = onFrame
        onFrame = { cpu, moving, at ->
            before?.invoke(cpu, moving, at)
            if (recording && moving) frames.add(Frame(cpu, at, JSONObject(Native.frameStats(handle) ?: "{}")))
        }
        fun step(i: Int) {
            if (i >= states.length()) { onFrame = before; done(results); return }
            frames.clear()
            recording = true
            goTo(i)
            main.postDelayed({
                recording = false
                results.add(summary("${states.getString(i - 1)} → ${states.getString(i)}", frames.toList()))
                step(i + 1)
            }, holdMs)
        }
        main.postDelayed({ step(1) }, holdMs)
    }

    private class Frame(val cpu: Double, val at: Long, val stats: JSONObject)

    private fun summary(label: String, f: List<Frame>): JSONObject {
        val gaps = f.zipWithNext { a, b -> (b.at - a.at) / 1e6 }
        val ms = if (f.size > 1) (f.last().at - f.first().at) / 1e9 else 0.0
        fun r(v: Double) = Math.round(v * 10) / 10.0
        fun spread(values: List<Double>): JSONObject {
            val v = values.filter { it.isFinite() }.sorted()
            fun at(q: Double) = if (v.isEmpty()) 0.0 else v[minOf(v.size - 1, (q * v.size).toInt())]
            return JSONObject().put("p50", r(at(0.5))).put("p95", r(at(0.95))).put("max", r(v.lastOrNull() ?: 0.0))
        }
        fun stat(key: String) = f.map { it.stats.optDouble(key, Double.NaN) }
        val out = JSONObject()
            .put("label", label)
            .put("frames", f.size)
            .put("fps", if (ms > 0) r((f.size - 1) / ms) else 0.0)
            .put("dropped", gaps.sumOf { maxOf(0, Math.round(it / (1000.0 / 60.0)).toInt() - 1) })
            .put("worst", r(gaps.maxOrNull() ?: 0.0))
            .put("worstAt", gaps.indexOf(gaps.maxOrNull() ?: 0.0) + 1)
            .put("cpu", spread(f.map { it.cpu }))
            .put("engine", spread(stat("engine")))
            .put("render", spread(stat("render")))
        for (key in listOf("draws", "uploaded", "rebuilt", "tessellated")) if (f.any { it.stats.has(key) }) out.put(key, spread(stat(key)))
        return out
    }

    /** Render on the next vsync. Requests coalesce: tile ranges arrive in bursts during a flight,
     * and rendering each on arrival would block the main thread for many frames. */
    private fun requestFrame() {
        if (!frameQueued) { frameQueued = true; Choreographer.getInstance().postFrameCallback(tick) }
    }

    fun loadDocument(json: String) { Native.loadDoc(handle, json); loaded(); }

    /** The app's own rows for the chart's data slots (CSV or JSON), by slot name. They never leave
     * the device; call any time — before the chart has loaded they wait for it. Returns false if
     * the chart is loaded and the rows lack the columns it needs (it keeps what it showed). */
    fun provideData(name: String, bytes: ByteArray): Boolean {
        hostData[name] = bytes
        if (!isLoaded) return true
        val ok = input { Native.provideSource(handle, name, bytes) }
        if (!ok) hostData.remove(name)
        requestFrame(); updateAccessibility()
        return ok
    }
    fun provideData(name: String, json: String): Boolean = provideData(name, json.toByteArray())

    /** A chart shipped inside the app: a `.datars` file in `assets/` (no network needed). */
    fun loadAsset(path: String) {
        val bytes = context.assets.open(path).use { it.readBytes() }
        Native.openFile(handle, bytes); loaded()
    }

    private val hostData = linkedMapOf<String, ByteArray>()
    private var isLoaded = false

    /** After any load: the view's size wins, rows handed in earlier fill their slots, first frame. */
    private fun loaded() {
        isLoaded = true
        for ((name, bytes) in hostData) Native.provideSource(handle, name, bytes)
        applySize(); renderFrame(); updateAccessibility()
        onLoad?.invoke()
    }

    /** Load a chart by URL: a manifest (an alias like `…/c/votes` or a pinned one) or a `.datars`
     * file. The runtime asks for the chunks its variant needs; they're fetched and handed over. */
    fun load(url: String) {
        dataBase = url
        io {
            val bytes = get(java.net.URL(url)) ?: return@io
            main.post {
                if (url.endsWith(".datars")) {
                    Native.openFile(handle, bytes); loaded()
                } else {
                    val need = JSONObject(Native.openManifest(handle, bytes) ?: "{}").optJSONArray("requests")
                    fetchChunks(url.substringBeforeLast("/c/"), need)
                }
            }
        }
    }

    private fun fetchChunks(base: String, need: org.json.JSONArray?) {
        if (need == null || need.length() == 0) { loaded(); return }
        val hashes = (0 until need.length()).map { need.getString(it) }
        io {
            val chunks = hashes.map { h -> h to get(java.net.URL("$base/chunks/${h.replace(":", "_")}")) }
            main.post {
                var next: org.json.JSONArray? = null
                for ((h, b) in chunks) if (b != null) next = JSONObject(Native.provideChunk(handle, h, b) ?: "{}").optJSONArray("requests")
                if (chunks.any { it.second == null }) return@post // a chunk failed: stay on the poster
                fetchChunks(base, next)
            }
        }
    }

    fun send(event: String) { if (input { Native.event(handle, event) }) requestFrame() }

    /** Scroll scrub: show program position [pos] (state + fraction toward the next). */
    fun seek(pos: Double) { input { Native.seek(handle, pos) }; requestFrame() }

    /** Pause or resume autoplay (e.g. when the view scrolls off-screen or animations are off). */
    fun setPlaying(on: Boolean) { input { Native.setPlaying(handle, on) }; requestFrame() }

    /** Jump to program state [index]; the transition plays on the next frames. */
    fun goTo(index: Int) { if (input { Native.gotoState(handle, index) }) { tipPopup?.dismiss(); requestFrame() } }

    /** The last frame's pixel identity as hex (compare with `datars render --hash`). */
    fun pixelHash(): String = java.lang.Long.toUnsignedString(Native.pixelHash(handle), 16).padStart(16, '0')

    /** "Vulkan 4×MSAA" while frames go to the GPU, null on CPU pixels. */
    fun gpuBackend(): String? = Native.gpuBackend(handle)

    /** The app's light or dark look; while the reader asks for more contrast (`followsContrast`),
     * the chart is in its high-contrast mode instead. */
    fun setDarkMode(dark: Boolean) { this.dark = dark; applyMode() }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        applySize()
        requestFrame()
    }

    /** The view's size in dp at its density. Also after every load: a document brings its own
     * default size, and the view's size wins. */
    private fun applySize() {
        if (width == 0 || height == 0) return
        val d = resources.displayMetrics.density.toDouble()
        Native.resize(handle, width / d, height / d, d)
    }

    /** Render one frame now; while something moves, the next is requested for the next vsync. */
    private fun renderFrame() {
        // No surface yet (or the view is off screen): the first one available asks for a frame.
        if (surface == null) return
        val now = now()
        val t0 = System.nanoTime()
        // Pictures are decoded by BitmapFactory before the frame that draws them (no pop-in).
        decodePictures()
        val next: Boolean
        if (gpu) {
            next = Native.drawFrame(handle, now)
            // A frame that lost the device fell back to CPU pixels.
            if (Native.gpuBackend(handle) == null) gpu = false
        } else {
            val px = Native.frame(handle, now) ?: return
            val w = Native.lastWidth(handle); val h = Native.lastHeight(handle)
            val bmp = bitmap?.takeIf { it.width == w && it.height == h } ?: Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888).also { bitmap = it }
            bmp.copyPixelsFromBuffer(ByteBuffer.wrap(px))  // RGBA8 straight alpha
            lockCanvas()?.let { c ->
                c.drawColor(android.graphics.Color.TRANSPARENT, android.graphics.PorterDuff.Mode.CLEAR)
                c.drawBitmap(bmp, null, android.graphics.Rect(0, 0, width, height), null)
                unlockCanvasAndPost(c)
            }
            next = Native.animating(handle)
        }
        val cpuMs = (System.nanoTime() - t0) / 1e6
        if (next) {
            val share = Work.note(cpuMs)
            if (Math.abs(share - workTold) > 0.04) { workTold = share; Native.setWorkScale(handle, share) }
        }
        onFrame?.invoke(cpuMs, next, if (vsyncNanos > 0) vsyncNanos else t0)
        fetchData()
        removeCallbacks(wake)
        if (next) {
            requestFrame()
        } else {
            // Nothing moving: sleep until the engine's next scheduled frame (autoplay, live refresh).
            val at = Native.wakeAt(handle)
            if (!at.isNaN()) postDelayed(wake, ((at - now) * 1000).toLong().coerceAtLeast(0))
        }
    }

    // Taps reach click listeners (and accessibility services) after the engine has seen them.
    override fun performClick(): Boolean = super.performClick()

    /** Every tap on the chart: the label of what it landed on (null: nothing) and where (view px).
     * Touch has no hover, so a tap is how a reader asks what a mark is — a line's value anywhere
     * along it. */
    var onTap: ((label: String?, x: Float, y: Float) -> Unit)? = null
    /** Show what a tap lands on in a small label above the finger, until the next tap or the next
     * step (default). Off, apps show [onTap]'s label their own way. */
    var showsTooltips = true
    /** Where the touch being tracked went down: released there, it's a tap. */
    private var downAt: Pair<Float, Float>? = null
    private var tipPopup: android.widget.PopupWindow? = null

    /** Engine-drawn selects (from the status): a tap on one opens the platform's own list. */
    private data class Picker(val signal: String, val label: String, val rect: android.graphics.RectF, val values: List<String>, val labels: List<String>, val current: String)
    private var pickers: List<Picker> = emptyList()
    private var pickerDown: Picker? = null

    /** The choices as Android offers a `<select>`: a dialog with one checked; a choice sets the
     * signal (its value as JSON). */
    private fun openPicker(p: Picker) {
        android.app.AlertDialog.Builder(context)
            .setTitle(p.label)
            .setSingleChoiceItems(p.labels.toTypedArray(), p.values.indexOf(p.current)) { dialog, which ->
                input { Native.setSignalJson(handle, p.signal, p.values[which]); null }
                dialog.dismiss()
                renderFrame(); updateAccessibility()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    override fun onTouchEvent(e: MotionEvent): Boolean {
        val d = resources.displayMetrics.density
        val x = (e.x / d).toDouble(); val y = (e.y / d).toDouble()
        // A press on a select is the platform's: the chart doesn't see it, and its release opens
        // the list.
        if (e.actionMasked == MotionEvent.ACTION_DOWN) pickerDown = pickers.firstOrNull { it.rect.contains(x.toFloat(), y.toFloat()) }
        pickerDown?.let { p ->
            if (e.actionMasked == MotionEvent.ACTION_UP) {
                pickerDown = null
                if (p.rect.contains(x.toFloat(), y.toFloat())) { openPicker(p); performClick() }
            } else if (e.actionMasked == MotionEvent.ACTION_CANCEL) pickerDown = null
            return true
        }
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> { downAt = e.x to e.y; input { Native.pointer(handle, 1, x, y) } }
            MotionEvent.ACTION_MOVE -> input { Native.pointer(handle, 0, x, y) }
            MotionEvent.ACTION_UP -> {
                // Released where it went down: a tap (kind 4), which inspects what it lands on
                // (with a finger's reach) as well as clicking it.
                val tap = downAt?.let { Math.hypot((e.x - it.first).toDouble(), (e.y - it.second).toDouble()) < 10 * d } ?: true
                downAt = null
                val raw = input { Native.pointer(handle, if (tap) 4 else 2, x, y) }
                val label = raw?.let { runCatching { org.json.JSONTokener(it).nextValue() as? String }.getOrNull() }
                if (tap) { onTap?.invoke(label, e.x, e.y); showTip(label, e.x, e.y) }
                renderFrame(); updateAccessibility(); sayChange(); performClick()
            }
            else -> { downAt = null; input { Native.pointer(handle, 3, x, y) } }
        }
        return true
    }

    /** A tap's label above the finger (it covers what's below it), within the view; nothing: hidden. */
    private fun showTip(label: String?, x: Float, y: Float) {
        if (!showsTooltips || label.isNullOrEmpty() || width == 0) { tipPopup?.dismiss(); return }
        val d = resources.displayMetrics.density
        val tv = (tipPopup?.contentView as? android.widget.TextView) ?: android.widget.TextView(context).apply {
            setTextColor(android.graphics.Color.WHITE)
            textSize = 12f
            setPadding((8 * d).toInt(), (4 * d).toInt(), (8 * d).toInt(), (4 * d).toInt())
            background = android.graphics.drawable.GradientDrawable().apply { setColor(0xEB141414.toInt()); cornerRadius = 4 * d }
        }
        tv.text = label
        tv.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.AT_MOST), View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED))
        val w = tv.measuredWidth; val h = tv.measuredHeight
        val left = (x - w / 2f).coerceIn(0f, (width - w).toFloat().coerceAtLeast(0f))
        val above = y - 28 * d - h
        val top = if (above >= 0) above else minOf(y + 28 * d, (height - h).toFloat())
        val at = IntArray(2).also { getLocationInWindow(it) }
        val px = at[0] + left.toInt(); val py = at[1] + top.toInt()
        val popup = tipPopup ?: android.widget.PopupWindow(tv, w, h, false).apply { isTouchable = false; isClippingEnabled = false }.also { tipPopup = it }
        if (popup.isShowing) popup.update(px, py, w, h) else { popup.width = w; popup.height = h; popup.showAtLocation(this, android.view.Gravity.NO_GRAVITY, px, py) }
    }

    /** One semantics item (or engine-drawn control) as TalkBack sees it. */
    private data class A11yItem(val label: String, val role: String, val rect: android.graphics.RectF?, val path: String, val actionable: Boolean,
                                val signal: String? = null, val min: Double = 0.0, val max: Double = 1.0, val step: Double = 0.0, val value: Double = 0.0,
                                /** What it says after its name: a data mark's value, the chart's summary. */
                                val said: String = "",
                                /** A checkbox's, switch's or radio's state (null: none of those). */
                                val checked: Boolean? = null,
                                /** How to operate a brush or a pannable view, in the chart's language. */
                                val hint: String = "")
    private var a11y: List<A11yItem> = emptyList()

    /** TalkBack reads the chart's content, not a picture, in the runtime's reading order: the
     * chart first (its name and summary), its controls (sliders adjustable ranges, selects the
     * platform's list), headings for its parts, an axis or a legend as one line, each data mark
     * with its value, clickable marks and controls as buttons — each with its frame. A new step
     * is announced. */
    private fun updateAccessibility() {
        val s = JSONObject(Native.accessibility(handle) ?: "{}")
        val out = mutableListOf<A11yItem>()
        val items = s.optJSONArray("readout") ?: org.json.JSONArray()
        phrases = s.optJSONObject("phrases")
        fun rect(it: JSONObject) = it.optJSONArray("rect")?.let { a -> if (a.length() == 4 && !a.isNull(0)) android.graphics.RectF(a.getDouble(0).toFloat(), a.getDouble(1).toFloat(), (a.getDouble(0) + a.getDouble(2)).toFloat(), (a.getDouble(1) + a.getDouble(3)).toFloat()) else null }
        // Engine-drawn controls, each with where it is: sliders as adjustable ranges, selects as
        // buttons that open the platform's list (and the list is what a tap on one opens too).
        val found = mutableListOf<Picker>()
        s.optJSONArray("controls")?.let { cs ->
            for (i in 0 until cs.length()) {
                val c = cs.getJSONObject(i)
                if (c.optString("kind") == "select") {
                    val r = rect(c) ?: continue
                    val os = c.optJSONArray("options") ?: continue
                    val values = (0 until os.length()).map { org.json.JSONObject.wrap(os.getJSONObject(it).opt("value")).let { v -> org.json.JSONArray().put(v).toString().removePrefix("[").removeSuffix("]") } }
                    val labels = (0 until os.length()).map { os.getJSONObject(it).optString("label") }
                    val current = org.json.JSONArray().put(org.json.JSONObject.wrap(c.opt("current"))).toString().removePrefix("[").removeSuffix("]")
                    found.add(Picker(c.optString("signal"), c.optString("label"), r, values, labels, current))
                    out.add(A11yItem(c.optString("label"), "select", r, "", true, c.optString("signal")))
                    continue
                }
                // A slider says its name, then its value as drawn ("32,000"), not the raw number.
                val l = c.optString("label")
                val cut = l.lastIndexOf(": ")
                val (name, value) = if (cut > 0) l.substring(0, cut) to l.substring(cut + 2) else l to ""
                out.add(A11yItem(name, "control", rect(c), "", false, c.optString("signal"), c.optDouble("min"), c.optDouble("max"), c.optDouble("step"), c.optDouble("value"), said = value))
            }
        }
        pickers = found
        val selectLabels = found.map { it.label }.toSet()
        for (i in 0 until minOf(items.length(), 400)) {
            val it = items.getJSONObject(i)
            val kind = it.optString("kind")
            // A select is its list above (the drawn box isn't a second one).
            if (kind == "button" && it.optString("label") in selectLabels) continue
            val clicks = kind == "button" || kind == "checkbox" || kind == "switch" || kind == "radio" || kind == "brush" || kind == "pan"
            // (How to operate it with TalkBack's gestures, not the keyboard's keys.)
            val hint = if (kind == "brush" || kind == "pan") s.optJSONObject("phrases")?.optString("$kind-hint-touch") ?: "" else ""
            val item = A11yItem(it.optString("label"), kind, rect(it), it.optString("path"), clicks, said = it.optString("value"), checked = if (it.has("checked")) it.optBoolean("checked") else null, hint = hint)
            // The chart first, then its controls, then its content.
            if (kind == "chart") out.add(0, item) else out.add(item)
        }
        // A story's steps, right after the chart: TalkBack has no other way to move it (a tap on
        // the view is the app's to give a meaning).
        val states = s.optJSONArray("states")?.length() ?: 0
        if (states > 1) {
            val at = out.indexOfFirst { it.role == "chart" } + 1
            val i = s.optInt("index")
            fun word(key: String, fallback: String) = s.optJSONObject("phrases")?.optString(key)?.takeIf { it.isNotEmpty() } ?: fallback
            val steps = listOfNotNull(
                if (i > 0) A11yItem(word("previous-step", "Previous step"), "step", null, "prev", true) else null,
                if (i < states - 1) A11yItem(word("next-step", "Next step"), "step", null, "next", true) else null,
            )
            out.addAll(at, steps)
        }
        a11y = out
        // The chart's name is its first item's (with its summary): the view itself says nothing
        // more, or TalkBack would read the name twice — once for the view, once for the chart.
        contentDescription = if (out.isEmpty()) "Chart" else null
        if (a11yFocused >= out.size) a11yFocused = View.NO_ID
        sendAccessibilityEvent(android.view.accessibility.AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
        announce(s)
    }

    /** The step TalkBack last heard about. */
    private var announced = -1

    /** What the reader's last tap, activation or adjustment changed, said once ("Pears, hidden.
     * Total: 69 kg."). */
    private fun sayChange(state: String? = null) {
        val change = Native.takeAnnouncement(handle)
        val text = listOfNotNull(state, change).filter { it.isNotEmpty() }.joinToString(" ")
        if (text.isEmpty()) return
        onAnnouncement?.invoke(text)
        @Suppress("DEPRECATION")
        announceForAccessibility(text)
    }

    /** What the reader's last tap, activation or adjustment changed, as TalkBack is told it: for an
     * app that also shows it. */
    var onAnnouncement: ((String) -> Unit)? = null

    /** State, narration, semantics, the theme's mode and tokens, diagnostics. */
    fun status(): JSONObject = JSONObject(Native.status(handle) ?: "{}")

    /** A new step, said once: its narration (title and text), else where the reader is ("Step 2 of
     * 4"). Not the first state: the chart's name and summary say what it is. */
    private fun announce(s: JSONObject) {
        val index = s.optInt("index")
        if (index == announced) return
        val first = announced < 0
        announced = index
        if (first) return
        val n = s.optJSONObject("narration")
        val said = listOfNotNull(n?.optString("title"), n?.optString("text")).filter { it.isNotEmpty() }.joinToString(". ")
        val count = s.optJSONArray("states")?.length() ?: 0
        val step = (s.optJSONObject("phrases")?.optString("step")?.takeIf { it.isNotEmpty() } ?: "Step {i} of {n}").replace("{i}", "${index + 1}").replace("{n}", "$count")
        val text = said.ifEmpty { if (count > 1) step else "" }
        @Suppress("DEPRECATION")
        if (text.isNotEmpty()) announceForAccessibility(text)
    }

    /** The virtual node TalkBack's focus is on (`View.NO_ID`: none). */
    private var a11yFocused = View.NO_ID
    /** The virtual node a finger exploring the screen is over. */
    private var a11yHovered = View.NO_ID

    /** An accessibility event from virtual node `id` (focus moved, a finger entered it). */
    private fun sendVirtual(id: Int, type: Int) {
        val item = a11y.getOrNull(id) ?: return
        @Suppress("DEPRECATION")
        val e = android.view.accessibility.AccessibilityEvent.obtain(type)
        e.setSource(this, id)
        e.packageName = context.packageName
        e.className = classOf(item)
        item.checked?.let { e.isChecked = it }
        e.contentDescription = if (item.said.isNotEmpty() && item.signal == null) "${item.label}, ${item.said.trimEnd('.')}" else item.label
        parent?.requestSendAccessibilityEvent(this, e)
    }

    /** The Android widget an item is to TalkBack (its role words and state come from it). */
    private fun classOf(item: A11yItem): String = when {
        item.role == "select" -> "android.widget.Spinner"
        item.signal != null -> "android.widget.SeekBar"
        item.role == "checkbox" -> "android.widget.CheckBox"
        item.role == "switch" -> "android.widget.Switch"
        item.role == "radio" -> "android.widget.RadioButton"
        item.actionable && item.role != "brush" && item.role != "pan" -> "android.widget.Button"
        else -> "android.view.View"
    }

    /** The words the view says around the chart's own, in the document's language (the status's
     * `phrases`). */
    private var phrases: JSONObject? = null

    /** Explore by touch: the item under the finger (the smallest box that holds it: a bar inside its
     * plot) is what TalkBack hears about. */
    override fun dispatchHoverEvent(e: MotionEvent): Boolean {
        val am = context.getSystemService(Context.ACCESSIBILITY_SERVICE) as android.view.accessibility.AccessibilityManager
        if (!am.isEnabled || !am.isTouchExplorationEnabled || a11y.isEmpty()) return super.dispatchHoverEvent(e)
        val d = resources.displayMetrics.density
        val (x, y) = e.x / d to e.y / d
        val at = when (e.actionMasked) {
            MotionEvent.ACTION_HOVER_EXIT -> View.NO_ID
            else -> a11y.indices.filter { a11y[it].role != "chart" && a11y[it].rect?.contains(x, y) == true }.minByOrNull { a11y[it].rect!!.width() * a11y[it].rect!!.height() }
                ?: a11y.indexOfFirst { it.role == "chart" }
        }
        if (at != a11yHovered) {
            if (a11yHovered != View.NO_ID) sendVirtual(a11yHovered, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_HOVER_EXIT)
            a11yHovered = at
            if (at != View.NO_ID) sendVirtual(at, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_HOVER_ENTER)
        }
        return true
    }

    private val provider = object : android.view.accessibility.AccessibilityNodeProvider() {
        // obtain() is deprecated from API 33; its replacements don't exist on the API 24 floor.
        @Suppress("DEPRECATION")
        override fun createAccessibilityNodeInfo(virtualViewId: Int): android.view.accessibility.AccessibilityNodeInfo? {
            val host = this@DatarsView
            if (virtualViewId == View.NO_ID) {
                val info = android.view.accessibility.AccessibilityNodeInfo.obtain(host)
                host.onInitializeAccessibilityNodeInfo(info)
                a11y.indices.forEach { info.addChild(host, it) }
                return info
            }
            val item = a11y.getOrNull(virtualViewId) ?: return null
            val info = android.view.accessibility.AccessibilityNodeInfo.obtain(host, virtualViewId)
            info.setParent(host)
            info.packageName = context.packageName
            info.text = item.label
            // (The chart's summary ends a sentence: TalkBack adds its own full stop.)
            if (item.said.isNotEmpty() && item.signal == null && item.role != "brush" && item.role != "pan") info.contentDescription = "${item.label}, ${item.said.trimEnd('.')}"
            info.isEnabled = true
            info.isVisibleToUser = true
            // Each item is a stop for TalkBack's swipes and for a finger exploring the screen.
            info.isFocusable = true
            if (android.os.Build.VERSION.SDK_INT >= 28) info.isScreenReaderFocusable = true
            info.isAccessibilityFocused = virtualViewId == a11yFocused
            info.addAction(if (virtualViewId == a11yFocused) android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_ACCESSIBILITY_FOCUS else android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_ACCESSIBILITY_FOCUS)
            val d = resources.displayMetrics.density
            val loc = IntArray(2).also { host.getLocationOnScreen(it) }
            val r = item.rect ?: android.graphics.RectF(0f, 0f, width / d, height / d)
            info.setBoundsInScreen(android.graphics.Rect((r.left * d).toInt() + loc[0], (r.top * d).toInt() + loc[1], (r.right * d).toInt() + loc[0], (r.bottom * d).toInt() + loc[1]))
            when {
                item.role == "select" -> {
                    // What a native select is to TalkBack: a spinner; a double-tap opens the list.
                    info.className = "android.widget.Spinner"
                    info.isClickable = true
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK)
                }
                item.signal != null -> {
                    info.className = "android.widget.SeekBar"
                    // Its value as drawn: what TalkBack says for it, not the range's raw number.
                    if (item.said.isNotEmpty() && android.os.Build.VERSION.SDK_INT >= 30) info.stateDescription = item.said
                    info.rangeInfo = android.view.accessibility.AccessibilityNodeInfo.RangeInfo.obtain(android.view.accessibility.AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT, item.min.toFloat(), item.max.toFloat(), item.value.toFloat())
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD)
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD)
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS)
                }
                item.role == "brush" || item.role == "pan" -> {
                    // What a finger drags, for TalkBack: a double-tap starts (and keeps) a selection
                    // or shows all of a view; scrolling forward and back extends a selection or
                    // zooms; a view's scroll left, right, up and down moves it; dismiss cancels.
                    // Adjustable, as a slider is: TalkBack's swipe up and down are scroll forward and
                    // back here; what it holds ("1 to 50") is what it says as its state.
                    info.className = "android.widget.SeekBar"
                    info.rangeInfo = android.view.accessibility.AccessibilityNodeInfo.RangeInfo.obtain(android.view.accessibility.AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT, 0f, 1f, 0.5f)
                    // (In the middle of its range: TalkBack swipes both ways only off the ends.)
                    if (android.os.Build.VERSION.SDK_INT >= 30) info.stateDescription = item.said
                    if (item.hint.isNotEmpty() && android.os.Build.VERSION.SDK_INT >= 26) info.hintText = item.hint
                    info.isClickable = true
                    listOf(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK,
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD,
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD,
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_DISMISS).forEach { info.addAction(it) }
                    if (item.role == "pan" && android.os.Build.VERSION.SDK_INT >= 23) listOf(
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_LEFT,
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_RIGHT,
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_UP,
                        android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_DOWN).forEach { info.addAction(it) }
                }
                item.actionable -> {
                    // A checkbox, switch or radio as Android's own: TalkBack says its state.
                    info.className = when (item.role) {
                        "checkbox" -> "android.widget.CheckBox"
                        "switch" -> "android.widget.Switch"
                        "radio" -> "android.widget.RadioButton"
                        else -> "android.widget.Button"
                    }
                    item.checked?.let { info.isCheckable = true; info.isChecked = it }
                    // A switch says on or off (TalkBack takes a switch's state from its description,
                    // as Android's own switches give it).
                    if (item.role == "switch" && item.checked != null && android.os.Build.VERSION.SDK_INT >= 30)
                        info.stateDescription = phrases?.optString(if (item.checked) "on" else "off")?.takeIf { it.isNotEmpty() } ?: if (item.checked) "On" else "Off"
                    info.isClickable = true
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK)
                }
                else -> {
                    info.className = "android.view.View"
                    // The chart and its parts' names: TalkBack's headings navigation.
                    if ((item.role == "chart" || item.role == "heading") && android.os.Build.VERSION.SDK_INT >= 28) info.isHeading = true
                }
            }
            return info
        }

        override fun performAction(virtualViewId: Int, action: Int, arguments: android.os.Bundle?): Boolean {
            val item = a11y.getOrNull(virtualViewId) ?: return false
            // TalkBack's focus: it moves here (and says the item), or leaves.
            if (action == android.view.accessibility.AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS) {
                if (a11yFocused == virtualViewId) return false
                val was = a11yFocused
                a11yFocused = virtualViewId
                if (was != View.NO_ID) sendVirtual(was, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
                sendVirtual(virtualViewId, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
                return true
            }
            if (action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) {
                if (a11yFocused != virtualViewId) return false
                a11yFocused = View.NO_ID
                sendVirtual(virtualViewId, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
                return true
            }
            val step = if (item.step > 0) item.step else (item.max - item.min) / 20
            val drag = item.role == "brush" || item.role == "pan"
            fun key(k: String) = input { Native.key(handle, item.path, k) }
            val done = when {
                item.role == "step" && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK -> input { Native.event(handle, item.path) }
                drag && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK -> key(if (item.role == "brush") "Enter" else "0")
                drag && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> if (item.role == "pan") key("+") else (key("ArrowRight") || key("Enter"))
                drag && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> if (item.role == "pan") key("-") else (key("ArrowLeft") || key("Enter"))
                drag && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_DISMISS -> key("Escape")
                drag && action == android.R.id.accessibilityActionScrollLeft -> key("ArrowLeft")
                drag && action == android.R.id.accessibilityActionScrollRight -> key("ArrowRight")
                drag && action == android.R.id.accessibilityActionScrollUp -> key("ArrowUp")
                drag && action == android.R.id.accessibilityActionScrollDown -> key("ArrowDown")
                action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK && item.role == "select" -> pickers.firstOrNull { it.label == item.label }?.let { openPicker(it); true } ?: false
                action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK && item.actionable -> input { Native.activate(handle, item.path) }
                item.signal != null && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> { input { Native.setSignalNum(handle, item.signal, minOf(item.max, item.value + step)) }; true }
                item.signal != null && action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> { input { Native.setSignalNum(handle, item.signal, maxOf(item.min, item.value - step)) }; true }
                item.signal != null && action == android.R.id.accessibilityActionSetProgress -> {
                    val v = arguments?.getFloat(android.view.accessibility.AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE) ?: return false
                    input { Native.setSignalNum(handle, item.signal, v.toDouble().coerceIn(item.min, item.max)) }; true
                }
                else -> false
            }
            if (done) {
                renderFrame(); updateAccessibility()
                // The item acted on, as it is now (a slider's new value, a switch's new state):
                // TalkBack's focus stays on it and hears what changed about it.
                val now = a11y.indexOfFirst { (item.signal != null && it.signal == item.signal) || (item.path.isNotEmpty() && it.path == item.path) }
                if (now >= 0) {
                    a11yFocused = now
                    @Suppress("DEPRECATION")
                    val e = android.view.accessibility.AccessibilityEvent.obtain(android.view.accessibility.AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
                    e.setSource(this@DatarsView, now)
                    e.packageName = context.packageName
                    e.contentChangeTypes = android.view.accessibility.AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT or (if (android.os.Build.VERSION.SDK_INT >= 30) android.view.accessibility.AccessibilityEvent.CONTENT_CHANGE_TYPE_STATE_DESCRIPTION else 0) or
                        // (CONTENT_CHANGE_TYPE_CHECKED, Android 15: TalkBack says a checked state's change from it.)
                        (if (android.os.Build.VERSION.SDK_INT >= 35 && item.checked != null) 0x2000 else 0)
                    parent?.requestSendAccessibilityEvent(this@DatarsView, e)
                    // A slider says its new value the way Android's own does: as a selection; a
                    // click is a click (a checkbox's, a switch's new state).
                    if (item.signal != null || (drag && action != android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK)) sendVirtual(now, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_SELECTED)
                    else if (action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK) sendVirtual(now, android.view.accessibility.AccessibilityEvent.TYPE_VIEW_CLICKED)
                }
                // A checkbox's, a switch's or a radio button's new state, said: TalkBack (15) says
                // nothing of its own when a virtual one is toggled.
                val toggled = a11y.getOrNull(now)?.takeIf { action == android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK && it.checked != null }
                val state = toggled?.let { t ->
                    val key = when (t.role) { "switch" -> if (t.checked == true) "on" else "off"; else -> if (t.checked == true) "checked" else "unchecked" }
                    (phrases?.optString(key)?.takeIf { it.isNotEmpty() } ?: key.replaceFirstChar { it.uppercase() }) + "."
                }
                // (A brush's step says its new range as its state, through the selection event:
                // the engine's words for it would say it twice.)
                val stepped = item.role == "brush" && (action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_FORWARD || action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD)
                if (stepped) Native.takeAnnouncement(handle) else sayChange(state)
            }
            return done
        }
    }

    override fun getAccessibilityNodeProvider(): android.view.accessibility.AccessibilityNodeProvider = provider

    override fun onDetachedFromWindow() {
        tipPopup?.dismiss()
        runCatching { context.contentResolver.unregisterContentObserver(contrastObserver) }
        if (android.os.Build.VERSION.SDK_INT >= 34) (contrastListener as? android.app.UiModeManager.ContrastChangeListener)?.let { context.getSystemService(android.app.UiModeManager::class.java)?.removeContrastChangeListener(it) }
        contrastListener = null
        super.onDetachedFromWindow()
    }

    protected fun finalize() { Native.viewFree(handle) }

    /** The app's share of the engines' per-frame work budgets (points built, tiles decoded, map
     * features styled, meshes tessellated and uploaded — `Native.setWorkScale`). They're sized for a
     * desktop, and a phone's CPU runs the same work several times slower, so views start at half.
     * Then: a moving frame that nearly missed its refresh cuts the share at once; the average of
     * the moving frames lowers it while they run past ~8 ms and raises it again when there's room.
     * Detail then arrives over more frames instead of frames dropping. One for the app: it's the
     * device that is slow, not a chart. (The web runtime's rule, `noteFrame`.) */
    private object Work {
        var share = 0.5
        private var frameMs = 0.0
        fun note(ms: Double): Double {
            frameMs = if (frameMs == 0.0) ms else frameMs * 0.85 + ms * 0.15
            share = when {
                ms > 14 -> maxOf(0.2, share * 0.7)
                frameMs > 8 -> maxOf(0.2, share * 0.9)
                frameMs < 4 && ms < 8 -> minOf(1.0, share * 1.05)
                else -> share
            }
            return share
        }
    }
}
