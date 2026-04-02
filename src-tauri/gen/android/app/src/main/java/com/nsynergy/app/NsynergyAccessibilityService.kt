package com.nsynergy.app

import android.accessibilityservice.AccessibilityService
import android.view.accessibility.AccessibilityEvent

/**
 * Accessibility service for capturing touch events and injecting
 * gestures on the Android device.
 *
 * This service is used in two modes:
 * 1. Trackpad mode: Captures touch on the phone screen and relays
 *    as mouse movement to the connected desktop.
 * 2. Injection mode: Receives mouse/keyboard events from a desktop
 *    and injects them as gestures/key events on the phone.
 *
 * Actual implementation will be added in Task #12 and #13.
 */
class NsynergyAccessibilityService : AccessibilityService() {

    companion object {
        @Volatile
        var instance: NsynergyAccessibilityService? = null
            private set
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // Will be implemented in Task #12
    }

    override fun onInterrupt() {
        // Required override
    }

    override fun onDestroy() {
        super.onDestroy()
        instance = null
    }
}
