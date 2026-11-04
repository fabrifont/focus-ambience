package com.fabrifont.focus.engine

import kotlinx.coroutines.flow.StateFlow

/** Sonido del catálogo. `theme` es el nombre del tema ("Generados" en la etapa 1). */
data class SoundItem(val id: String, val name: String, val theme: String)

data class TimerInfo(val totalSecs: Long, val remainingSecs: Long, val paused: Boolean)

/** Estado que ve la UI; propio de la app para no filtrar los tipos del puente. */
data class EngineState(
    val sounds: List<SoundItem> = emptyList(),
    val volumes: Map<String, Float> = emptyMap(),
    val active: Set<String> = emptySet(),
    val paused: Boolean = false,
    val master: Float = 1f,
    val timer: TimerInfo? = null,
) {
    /** Suena algo ahora mismo. */
    val audible: Boolean get() = active.isNotEmpty() && !paused

    /** Hay una sesión: algo prendido (aunque en pausa) o el timer corriendo. */
    val busy: Boolean get() = active.isNotEmpty() || timer != null

    /** Algo avanza: sonidos sonando, o (sin sonidos) el timer sin pausar. */
    val running: Boolean
        get() = if (active.isNotEmpty()) !paused else timer?.paused == false
}

sealed interface EngineCommand {
    data class SetActive(val id: String, val active: Boolean) : EngineCommand
    data class Toggle(val id: String) : EngineCommand
    data class SetVolume(val id: String, val volume: Float) : EngineCommand
    data class SetMaster(val volume: Float) : EngineCommand

    /** Alterna pausa/sigue de los sonidos y del timer juntos. */
    data object PauseAll : EngineCommand

    /** Apaga los sonidos y cancela el timer. */
    data object StopAll : EngineCommand

    /** `null` cancela. */
    data class Timer(val minutes: Int?) : EngineCommand
    data object ToggleTimerPause : EngineCommand
}

interface FocusEngine {
    val state: StateFlow<EngineState>

    /** Se llama (en un hilo cualquiera) cuando el timer llega a cero. */
    var timerFinishedCallback: (() -> Unit)?

    fun send(cmd: EngineCommand)

    /** Arranca el hilo que produce el audio (lo llama el servicio). */
    fun startAudio()
    fun stopAudio()
}
