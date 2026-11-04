package com.fabrifont.focus.engine

import kotlinx.coroutines.flow.StateFlow

/** Sonido del catálogo. `theme` es el nombre del tema ("Generados" en la etapa 1). */
data class SoundItem(val id: String, val name: String, val theme: String)

/** Mezcla guardada: id de sonido -> volumen (0..1). */
data class PresetItem(val name: String, val layers: Map<String, Float>)

/** La meditación de una rutina. Android no la edita, pero la conserva al guardar. `minutes` null = libre. */
data class MeditationSlot(val minutes: Int?)

data class RoutineItem(val name: String, val tasks: List<String>, val meditation: MeditationSlot? = null)

/** Volumen que se muestra mientras el motor no tiene uno guardado para el sonido. */
const val DEFAULT_VOLUME = 0.5f

data class TimerInfo(val totalSecs: Long, val remainingSecs: Long, val paused: Boolean)

/** Estado que ve la UI; propio de la app para no filtrar los tipos del puente. */
data class EngineState(
    val sounds: List<SoundItem> = emptyList(),
    val volumes: Map<String, Float> = emptyMap(),
    val active: Set<String> = emptySet(),
    val paused: Boolean = false,
    val master: Float = 1f,
    val timer: TimerInfo? = null,
    val presets: List<PresetItem> = emptyList(),
) {
    fun volumeOf(id: String): Float = volumes[id] ?: DEFAULT_VOLUME

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

    /** Pausa los sonidos y el timer juntos. */
    data object PauseBoth : EngineCommand

    /** Sigue los sonidos y el timer juntos. */
    data object ResumeBoth : EngineCommand

    /** Apaga los sonidos y cancela el timer. */
    data object StopAll : EngineCommand

    /** `null` cancela. */
    data class Timer(val minutes: Int?) : EngineCommand
    data object ToggleTimerPause : EngineCommand

    /** Pausa solo el ambiente (y una meditación sonando); el timer sigue. */
    data object PauseAmbient : EngineCommand
    data object ResumeAmbient : EngineCommand

    data class LoadPreset(val name: String) : EngineCommand
    data class DeletePreset(val name: String) : EngineCommand
    data class StorePreset(val name: String, val layers: Map<String, Float>) : EngineCommand
}

interface FocusEngine {
    val state: StateFlow<EngineState>

    /** Se llama (en un hilo cualquiera) cuando el timer llega a cero. */
    var timerFinishedCallback: (() -> Unit)?

    fun send(cmd: EngineCommand)

    // Rutinas: leen y escriben routines.toml directo (no es audio). Tiran excepción si el archivo está roto.
    fun routines(): List<RoutineItem>
    fun saveRoutines(routines: List<RoutineItem>)

    /** ¿Ya hay otra rutina con ese nombre? `skip` = índice de la que se está editando. */
    fun routineNameTaken(routines: List<RoutineItem>, name: String, skip: Int?): Boolean
    fun lastRoutine(): String?
    fun setLastRoutine(name: String?)
}

/** Pausa si algo avanza y sigue si no. */
fun FocusEngine.togglePause() = send(if (state.value.running) EngineCommand.PauseBoth else EngineCommand.ResumeBoth)
