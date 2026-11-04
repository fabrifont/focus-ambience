package com.fabrifont.focus.ui

import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState

/**
 * El armado de un preset suena en vivo sobre el mismo motor. Al entrar se guarda lo que sonaba
 * (y se silencia); al salir —se haya guardado o no— se restaura. Vive a nivel proceso para
 * sobrevivir a una rotación de pantalla en el medio.
 */
object PresetPreview {
    private class Snapshot(val active: Set<String>, val volumes: Map<String, Float>, val paused: Boolean)

    private var snap: Snapshot? = null

    fun begin(state: EngineState, send: (EngineCommand) -> Unit) {
        if (snap != null) return
        snap = Snapshot(state.active, state.volumes, state.paused)
        for (id in state.active) send(EngineCommand.SetActive(id, false))
    }

    fun end(state: EngineState, send: (EngineCommand) -> Unit) {
        val s = snap ?: return
        snap = null
        for (sound in state.sounds) {
            if (sound.id !in s.active && sound.id in state.active) send(EngineCommand.SetActive(sound.id, false))
        }
        for ((id, v) in s.volumes) send(EngineCommand.SetVolume(id, v))
        for (id in s.active) send(EngineCommand.SetActive(id, true))
        if (s.paused && s.active.isNotEmpty()) send(EngineCommand.PauseAmbient)
    }
}
