package dev.datars.sample

import android.app.Activity
import android.os.Bundle
import android.util.Log
import dev.datars.DatarsView

/** One DatarsView showing the chart at `url` (an intent extra; default: `datars serve` through
 * `adb reverse`). Tap the chart to step through its states. With the `bench` extra it steps through
 * every state by itself instead, timing each transition, and logs one `DATARS-BENCH` line per
 * transition (scripts/bench-native.sh). */
class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val view = DatarsView(this)
        setContentView(view)
        val url = intent.getStringExtra("url") ?: "http://127.0.0.1:8791/c/votes"
        if (intent.getBooleanExtra("bench", false)) {
            val chart = url.substringAfterLast('/')
            view.onLoad = {
                view.runBenchmark { results ->
                    for (r in results) Log.i("datars", "DATARS-BENCH " + r.put("chart", chart))
                    Log.i("datars", "DATARS-BENCH-DONE $chart ${view.gpuBackend() ?: "CPU pixels"}")
                }
            }
        }
        if (intent.getBooleanExtra("ax", false)) watchAccessibility(view, intent.getStringExtra("act") ?: "")
        view.load(url)
        view.setOnClickListener { view.send("next") }
    }

    /** The `ax` extra: what TalkBack is told when something changes (`DATARS-SAID`), and every two
     * seconds the reader's settings as the chart follows them (`DATARS-SETTINGS`: the mode, the bold
     * text weight boost, the label size). The `act` extra (`"range selection:click,range
     * selection:forward"`) runs TalkBack's actions on the first node whose text has those words. */
    private fun watchAccessibility(view: DatarsView, act: String) {
        view.onAnnouncement = { Log.i("datars", "DATARS-SAID $it") }
        val h = android.os.Handler(mainLooper)
        var left = 30
        val tick = object : Runnable {
            override fun run() {
                val t = view.status().optJSONObject("tokens")
                val tokens = t?.optJSONObject("tokens")
                Log.i("datars", "DATARS-SETTINGS view=${System.identityHashCode(view)} mode=${t?.optString("mode")} boost=${tokens?.optDouble("text.weight-boost", 0.0)} label=${tokens?.optDouble("size.label")}")
                if (--left > 0 && !isDestroyed) h.postDelayed(this, 2000)
            }
        }
        view.onLoad = {
            h.postDelayed({
                val provider = view.accessibilityNodeProvider
                for (step in act.split(',').filter { it.contains(':') }) {
                    val (words, action) = step.split(':', limit = 2)
                    val id = (0 until 2000).firstOrNull { provider.createAccessibilityNodeInfo(it)?.text?.contains(words) == true } ?: continue
                    val code = when (action) {
                        "click" -> android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK
                        "forward" -> android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                        "backward" -> android.view.accessibility.AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD
                        "dismiss" -> android.view.accessibility.AccessibilityNodeInfo.ACTION_DISMISS
                        else -> continue
                    }
                    val done = provider.performAction(id, code, null)
                    Log.i("datars", "DATARS-ACT $step → $done ${provider.createAccessibilityNodeInfo(id)?.contentDescription}")
                }
                h.post(tick)
            }, 3000)
        }
    }
}
