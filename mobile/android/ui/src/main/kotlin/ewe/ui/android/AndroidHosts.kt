package ewe.ui.android

import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.view.HapticFeedbackConstants
import android.view.View
import ewe.ui.NativeCommandId
import ewe.ui.runtime.*

class MainGapTimer(private val handler: Handler = Handler(Looper.getMainLooper())) : GapTimer {
    override fun schedule(delayMs: Long, callback: () -> Unit): Cancel {
        val task = Runnable(callback)
        handler.postDelayed(task, delayMs)
        return Cancel { handler.removeCallbacks(task) }
    }
}

class SystemCommandHost(private val view: View) : CommandHost {
    override fun run(command: UShort, args: Map<String, Property>): Boolean {
        val context = view.context
        return when (command) {
            NativeCommandId.HAPTIC ->
                view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
            NativeCommandId.OPEN_URL -> {
                val url = args["url"]?.label ?: return false
                val uri = Uri.parse(url)
                if (uri.scheme !in listOf("https", "http")) return false
                context.startActivity(Intent(Intent.ACTION_VIEW, uri))
                true
            }
            NativeCommandId.SHARE -> {
                val text = args["text"]?.label ?: return false
                val share =
                    Intent(Intent.ACTION_SEND)
                        .setType("text/plain")
                        .putExtra(Intent.EXTRA_TEXT, text)
                context.startActivity(Intent.createChooser(share, null))
                true
            }
            else -> false
        }
    }
}
