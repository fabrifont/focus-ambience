package com.fabrifont.focus.engine

import android.content.Context

/** Le pasa el contexto de Android al motor Rust (lo necesita cpal/AAudio para abrir el audio). */
object NativeInit {
    init {
        System.loadLibrary("focus_ffi")
    }

    @JvmStatic
    external fun init(context: Context)
}
