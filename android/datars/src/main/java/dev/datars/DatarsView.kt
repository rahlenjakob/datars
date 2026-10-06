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
        Native.setWorkScale(it, Work.share)
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

    fun setDarkMode(dark: Boolean) { Native.setMode(handle, if (dark) 1 else 0); requestFrame() }

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
                renderFrame(); updateAccessibility(); performClick()
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
                                val signal: String? = null, val min: Double = 0.0, val max: Double = 1.0, val step: Double = 0.0, val value: Double = 0.0)
    private var a11y: List<A11yItem> = emptyList()

    /** TalkBack reads the engine's semantics — the chart's content, not a picture — with frames;
     * interactive marks are buttons, engine-drawn sliders are adjustable ranges. */
    private fun updateAccessibility() {
        val s = JSONObject(Native.status(handle) ?: "{}")
        val out = mutableListOf<A11yItem>()
        val items = s.optJSONArray("semantics") ?: org.json.JSONArray()
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
                out.add(A11yItem(c.optString("label"), "control", rect(c), "", false, c.optString("signal"), c.optDouble("min"), c.optDouble("max"), c.optDouble("step"), c.optDouble("value")))
            }
        }
        pickers = found
        val selectLabels = found.map { it.label }.toSet()
        for (i in 0 until minOf(items.length(), 400)) {
            val it = items.getJSONObject(i)
            // A clicked control (a switch, a checkbox, a button) is a button; a dragged one (a
            // slider's thumb) is its adjustable range above; a select is its list above.
            if (it.optString("role") == "control" && (!it.optBoolean("actionable") || it.optString("label") in selectLabels)) continue
            out.add(A11yItem(it.optString("label"), it.optString("role"), rect(it), it.optString("path"), it.optBoolean("actionable")))
        }
        a11y = out
        contentDescription = out.firstOrNull { it.role == "title" }?.label ?: out.firstOrNull()?.label
        sendAccessibilityEvent(android.view.accessibility.AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
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
            info.isEnabled = true
            info.isVisibleToUser = true
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
                    info.rangeInfo = android.view.accessibility.AccessibilityNodeInfo.RangeInfo.obtain(android.view.accessibility.AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT, item.min.toFloat(), item.max.toFloat(), item.value.toFloat())
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD)
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD)
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS)
                }
                item.actionable -> {
                    info.className = "android.widget.Button"
                    info.isClickable = true
                    info.addAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK)
                }
                else -> info.className = "android.view.View"
            }
            return info
        }

        override fun performAction(virtualViewId: Int, action: Int, arguments: android.os.Bundle?): Boolean {
            val item = a11y.getOrNull(virtualViewId) ?: return false
            val step = if (item.step > 0) item.step else (item.max - item.min) / 20
            val done = when {
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
            if (done) { renderFrame(); updateAccessibility() }
            return done
        }
    }

    override fun getAccessibilityNodeProvider(): android.view.accessibility.AccessibilityNodeProvider = provider

    override fun onDetachedFromWindow() { tipPopup?.dismiss(); super.onDetachedFromWindow() }

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
