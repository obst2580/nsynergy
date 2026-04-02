# Tauri / nsynergy ProGuard rules
-keep class com.nsynergy.app.** { *; }
-keep class app.tauri.** { *; }

# Keep JNI methods
-keepclassmembers class * {
    native <methods>;
}
