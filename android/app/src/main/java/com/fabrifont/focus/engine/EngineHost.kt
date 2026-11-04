package com.fabrifont.focus.engine

import android.content.Context

/**
 * Un solo motor por proceso. Se crea perezoso (el estado existe antes de que arranque el
 * servicio, así la UI puede mostrar la lista), pero el audio y su ciclo de vida son del
 * [com.fabrifont.focus.PlaybackService].
 */
object EngineHost {
    @Volatile
    private var engine: FocusEngine? = null

    fun get(context: Context): FocusEngine =
        engine ?: synchronized(this) {
            engine ?: RustEngine(context.applicationContext).also { engine = it }
        }
}
