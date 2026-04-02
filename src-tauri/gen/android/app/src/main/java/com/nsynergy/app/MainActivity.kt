package com.nsynergy.app

import android.os.Bundle

/**
 * Main activity for the nsynergy Android app.
 * Hosts the Tauri WebView and registers native plugins.
 */
class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
    }
}
