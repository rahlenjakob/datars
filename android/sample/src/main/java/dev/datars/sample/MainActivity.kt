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
        view.load(url)
        view.setOnClickListener { view.send("next") }
    }
}
