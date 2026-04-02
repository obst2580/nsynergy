package com.nsynergy.app

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.Context
import android.content.Intent
import android.graphics.Path
import android.os.Build
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

/**
 * Tauri Kotlin plugin bridging Rust commands to Android native APIs.
 *
 * Responsibilities:
 * - Check/request accessibility service permission
 * - Forward touch events from the WebView touchpad UI to Rust
 * - Receive injection commands from Rust and dispatch via AccessibilityService
 */
@TauriPlugin
class NsynergyPlugin(private val activity: android.app.Activity) : Plugin(activity) {

    /**
     * Check whether the NsynergyAccessibilityService is currently enabled.
     */
    @Command
    fun isAccessibilityEnabled(invoke: Invoke) {
        val enabled = isServiceEnabled(activity, NsynergyAccessibilityService::class.java)
        invoke.resolve(mapOf("enabled" to enabled))
    }

    /**
     * Open the system Accessibility Settings so the user can enable our service.
     */
    @Command
    fun openAccessibilitySettings(invoke: Invoke) {
        val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        activity.startActivity(intent)
        invoke.resolve()
    }

    /**
     * Receive a touch move event from the React touchpad UI.
     * The WebView captures touch coordinates and sends them here.
     *
     * Expected args: { "x": number, "y": number }
     */
    @Command
    fun sendTouchMove(invoke: Invoke) {
        val x = invoke.getDouble("x") ?: run {
            invoke.reject("missing x")
            return
        }
        val y = invoke.getDouble("y") ?: run {
            invoke.reject("missing y")
            return
        }
        // Forward to Rust via JNI bridge
        nativeBridgeMouseMove(x, y)
        invoke.resolve()
    }

    /**
     * Receive a tap (click) event from the React touchpad UI.
     *
     * Expected args: { "button": number (0=left,1=right,2=middle), "pressed": boolean }
     */
    @Command
    fun sendTap(invoke: Invoke) {
        val button = invoke.getInt("button") ?: 0
        val pressed = invoke.getBoolean("pressed") ?: true
        nativeBridgeMouseButton(button, pressed)
        invoke.resolve()
    }

    /**
     * Receive a scroll event from the React touchpad UI.
     *
     * Expected args: { "dx": number, "dy": number }
     */
    @Command
    fun sendScroll(invoke: Invoke) {
        val dx = invoke.getDouble("dx") ?: 0.0
        val dy = invoke.getDouble("dy") ?: 0.0
        nativeBridgeScroll(dx, dy)
        invoke.resolve()
    }

    /**
     * Receive a key event from the React virtual keyboard UI.
     *
     * Expected args: { "code": number, "pressed": boolean }
     */
    @Command
    fun sendKey(invoke: Invoke) {
        val code = invoke.getInt("code") ?: run {
            invoke.reject("missing code")
            return
        }
        val pressed = invoke.getBoolean("pressed") ?: true
        nativeBridgeKey(code, pressed)
        invoke.resolve()
    }

    /**
     * Inject a tap gesture at the given screen coordinates via AccessibilityService.
     * Used when receiving injection commands from a connected desktop.
     *
     * Expected args: { "x": number, "y": number, "duration": number (ms) }
     */
    @Command
    fun injectTap(invoke: Invoke) {
        val service = NsynergyAccessibilityService.instance
        if (service == null) {
            invoke.reject("accessibility service not running")
            return
        }

        val x = invoke.getFloat("x") ?: run {
            invoke.reject("missing x")
            return
        }
        val y = invoke.getFloat("y") ?: run {
            invoke.reject("missing y")
            return
        }
        val duration = invoke.getLong("duration") ?: 100L

        val path = Path().apply { moveTo(x, y) }
        val stroke = GestureDescription.StrokeDescription(path, 0, duration)
        val gesture = GestureDescription.Builder().addStroke(stroke).build()

        service.dispatchGesture(
            gesture,
            object : AccessibilityService.GestureResultCallback() {
                override fun onCompleted(gestureDescription: GestureDescription?) {
                    invoke.resolve()
                }
                override fun onCancelled(gestureDescription: GestureDescription?) {
                    invoke.reject("gesture cancelled")
                }
            },
            null
        )
    }

    /**
     * Inject a swipe gesture via AccessibilityService.
     *
     * Expected args: { "startX": number, "startY": number,
     *                  "endX": number, "endY": number, "duration": number (ms) }
     */
    @Command
    fun injectSwipe(invoke: Invoke) {
        val service = NsynergyAccessibilityService.instance
        if (service == null) {
            invoke.reject("accessibility service not running")
            return
        }

        val startX = invoke.getFloat("startX") ?: run { invoke.reject("missing startX"); return }
        val startY = invoke.getFloat("startY") ?: run { invoke.reject("missing startY"); return }
        val endX = invoke.getFloat("endX") ?: run { invoke.reject("missing endX"); return }
        val endY = invoke.getFloat("endY") ?: run { invoke.reject("missing endY"); return }
        val duration = invoke.getLong("duration") ?: 300L

        val path = Path().apply {
            moveTo(startX, startY)
            lineTo(endX, endY)
        }
        val stroke = GestureDescription.StrokeDescription(path, 0, duration)
        val gesture = GestureDescription.Builder().addStroke(stroke).build()

        service.dispatchGesture(
            gesture,
            object : AccessibilityService.GestureResultCallback() {
                override fun onCompleted(gestureDescription: GestureDescription?) {
                    invoke.resolve()
                }
                override fun onCancelled(gestureDescription: GestureDescription?) {
                    invoke.reject("gesture cancelled")
                }
            },
            null
        )
    }

    // ---- JNI native methods ----
    // These call into the Rust side via the nsynergy-core platform bridge.

    private external fun nativeBridgeMouseMove(x: Double, y: Double)
    private external fun nativeBridgeMouseButton(button: Int, pressed: Boolean)
    private external fun nativeBridgeScroll(dx: Double, dy: Double)
    private external fun nativeBridgeKey(code: Int, pressed: Boolean)

    companion object {
        init {
            System.loadLibrary("nsynergy_tauri_lib")
        }

        /**
         * Check if a specific AccessibilityService is enabled.
         */
        fun isServiceEnabled(context: Context, serviceClass: Class<out AccessibilityService>): Boolean {
            val enabledServices = Settings.Secure.getString(
                context.contentResolver,
                Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
            ) ?: return false

            val componentName = "${context.packageName}/${serviceClass.canonicalName}"
            return enabledServices.split(':').any {
                it.equals(componentName, ignoreCase = true)
            }
        }
    }
}

// Extension helpers to extract typed values from Invoke args.
// Tauri v2 Kotlin plugin API exposes arguments via JSObject.

private fun Invoke.getDouble(key: String): Double? {
    return try {
        this.parseArgs(DoubleArg::class.java)?.let {
            val field = it.javaClass.getDeclaredField(key)
            field.isAccessible = true
            field.getDouble(it)
        }
    } catch (e: Exception) {
        null
    }
}

private fun Invoke.getFloat(key: String): Float? {
    return getDouble(key)?.toFloat()
}

private fun Invoke.getInt(key: String): Int? {
    return try {
        this.parseArgs(IntArg::class.java)?.let {
            val field = it.javaClass.getDeclaredField(key)
            field.isAccessible = true
            field.getInt(it)
        }
    } catch (e: Exception) {
        null
    }
}

private fun Invoke.getBoolean(key: String): Boolean? {
    return try {
        this.parseArgs(BoolArg::class.java)?.let {
            val field = it.javaClass.getDeclaredField(key)
            field.isAccessible = true
            field.getBoolean(it)
        }
    } catch (e: Exception) {
        null
    }
}

private fun Invoke.getLong(key: String): Long? {
    return getDouble(key)?.toLong()
}

// Dummy arg classes for parseArgs
private class DoubleArg
private class IntArg
private class BoolArg
