package com.fabrifont.focus.engine

import android.content.Context
import android.util.Log
import java.io.File
import java.util.concurrent.Executors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.focus_ffi.Command
import uniffi.focus_ffi.CoreException
import uniffi.focus_ffi.Listener
import uniffi.focus_ffi.Player
import uniffi.focus_ffi.Routine
import uniffi.focus_ffi.RoutineMeditation
import uniffi.focus_ffi.State

/** Envuelve el Player de core (vía focus_ffi): el audio lo produce core con AAudio. */
class RustEngine(context: Context) : FocusEngine {
    private val player = run {
        NativeInit.init(context.applicationContext)
        Player(File(context.filesDir, "library").path)
    }
    private val _state = MutableStateFlow(player.state().toUi())
    override val state: StateFlow<EngineState> = _state.asStateFlow()

    @Volatile
    override var timerFinishedCallback: (() -> Unit)? = null

    // Los comandos van por un hilo aparte: guardar volúmenes escribe a disco.
    private val commands = Executors.newSingleThreadExecutor { Thread(it, "focus-cmd") }

    init {
        player.setListener(object : Listener {
            override fun onState(state: State) {
                _state.value = state.toUi()
            }

            override fun onTimerFinished() {
                timerFinishedCallback?.invoke()
            }

            override fun onNotice(text: String) {
                Log.i("focus", text)
            }
        })
    }

    override fun send(cmd: EngineCommand) {
        val c = cmd.toCore()
        commands.execute {
            try {
                player.send(c)
            } catch (e: CoreException) {
                Log.w("focus", "comando rechazado: $e")
            }
        }
    }

    override fun routines(): List<RoutineItem> = player.routines().map {
        RoutineItem(it.name, it.tasks, it.meditation?.let { m -> MeditationSlot(m.minutes?.toInt()) })
    }

    override fun saveRoutines(routines: List<RoutineItem>) = player.saveRoutines(routines.map { it.toCore() })

    override fun routineNameTaken(routines: List<RoutineItem>, name: String, skip: Int?): Boolean =
        player.routineNameTaken(routines.map { it.toCore() }, name, skip?.toUInt())

    override fun lastRoutine(): String? = player.lastRoutine()

    override fun setLastRoutine(name: String?) = player.setLastRoutine(name)
}

private fun RoutineItem.toCore() =
    Routine(name, tasks, meditation?.let { RoutineMeditation(it.minutes?.toUInt()) })

private fun State.toUi() = EngineState(
    sounds = sounds.map { SoundItem(it.id, it.name, it.theme) },
    volumes = volumes,
    active = active.toSet(),
    paused = paused,
    master = master,
    timer = timer?.let { TimerInfo(it.totalSecs.toLong(), it.remainingSecs.toLong(), it.paused) },
    presets = presets.map { PresetItem(it.name, it.layers) },
)

private fun EngineCommand.toCore(): Command = when (this) {
    is EngineCommand.SetActive -> Command.SetActive(id, active)
    is EngineCommand.Toggle -> Command.Toggle(id)
    is EngineCommand.SetVolume -> Command.SetVolume(id, volume)
    is EngineCommand.SetMaster -> Command.SetMaster(volume)
    EngineCommand.PauseBoth -> Command.PauseBoth
    EngineCommand.ResumeBoth -> Command.ResumeBoth
    EngineCommand.StopAll -> Command.StopAll
    is EngineCommand.Timer -> Command.Timer(minutes?.toUInt())
    EngineCommand.ToggleTimerPause -> Command.ToggleTimerPause
    EngineCommand.PauseAmbient -> Command.PauseAll
    EngineCommand.ResumeAmbient -> Command.ResumeAll
    is EngineCommand.LoadPreset -> Command.LoadPreset(name)
    is EngineCommand.DeletePreset -> Command.DeletePreset(name)
    is EngineCommand.StorePreset -> Command.StorePreset(name, layers)
}
