package com.fabrifont.focus.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.fabrifont.focus.Prefs
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.FocusEngine
import com.fabrifont.focus.engine.RoutineItem
import com.fabrifont.focus.ui.overlays.LoadPresetSheet
import com.fabrifont.focus.ui.overlays.OptionsSheet
import com.fabrifont.focus.ui.overlays.PreStartDialog
import com.fabrifont.focus.ui.overlays.TimerSheet
import com.fabrifont.focus.ui.screens.ChooseRoutineScreen
import com.fabrifont.focus.ui.screens.FocusScreen
import com.fabrifont.focus.ui.screens.HomeScreen
import com.fabrifont.focus.ui.screens.PresetBuilderScreen
import com.fabrifont.focus.ui.screens.PresetsConfigScreen
import com.fabrifont.focus.ui.screens.RoutineEditorScreen
import com.fabrifont.focus.ui.screens.RoutineRunScreen
import com.fabrifont.focus.ui.screens.RoutinesConfigScreen
import com.fabrifont.focus.ui.screens.SettingsScreen
import com.fabrifont.focus.ui.screens.SoundsConfigScreen
import com.fabrifont.focus.ui.theme.FocusTheme
import com.fabrifont.focus.ui.theme.GeneratedPalette
import com.fabrifont.focus.ui.theme.IdlePalette

private enum class Screen {
    Home, Focus, Settings, SoundsConfig, PresetsConfig, PresetBuilder,
    RoutinesConfig, RoutineEditor, ChooseRoutine, RoutineRun,
}

private enum class Sheet { None, Options, Timer, LoadPreset }

@Composable
fun FocusApp(engine: FocusEngine, prefs: Prefs) {
    val state by engine.state.collectAsState()
    val routines = remember(engine) { RoutinesModel(engine) }

    // Etapa 1: todo lo que suena es tema Generado; sin sonido, el crepúsculo neutro.
    val mix by animateFloatAsState(if (state.audible) 1f else 0f, tween(900), label = "palette")
    val palette = remember(mix) { IdlePalette.lerp(GeneratedPalette, mix) }

    var screen by rememberSaveable { mutableStateOf(Screen.Home) }
    var sheet by rememberSaveable { mutableStateOf(Sheet.None) }
    var preStart by rememberSaveable { mutableStateOf(false) }
    var editIndex by rememberSaveable { mutableStateOf<Int?>(null) }
    var runName by rememberSaveable { mutableStateOf<String?>(null) }
    // Durante una rutina el ambiente se pausa; si lo pausamos nosotros, lo devolvemos al salir.
    var ambientHeld by rememberSaveable { mutableStateOf(false) }
    val send = remember(engine) { { cmd: EngineCommand -> engine.send(cmd) } }

    fun enterFocus() {
        // Con una sesión en curso (algo prendido o timer) se entra directo.
        if (state.busy) screen = Screen.Focus else preStart = true
    }

    fun startRoutine(r: RoutineItem) {
        routines.markStarted(r.name)
        ambientHeld = state.audible
        if (ambientHeld) send(EngineCommand.PauseAmbient)
        runName = r.name
        screen = Screen.RoutineRun
    }

    fun leaveRun() {
        if (ambientHeld) send(EngineCommand.ResumeAmbient)
        ambientHeld = false
        runName = null
        screen = Screen.Home
    }

    fun leaveBuilder() {
        PresetPreview.end(state, send)
        screen = Screen.PresetsConfig
    }

    BackHandler(enabled = screen != Screen.Home) {
        when (screen) {
            Screen.SoundsConfig, Screen.PresetsConfig, Screen.RoutinesConfig -> screen = Screen.Settings
            Screen.PresetBuilder -> leaveBuilder()
            Screen.RoutineEditor -> screen = Screen.RoutinesConfig
            Screen.RoutineRun -> leaveRun()
            else -> screen = Screen.Home
        }
    }

    FocusTheme(palette) {
        Box(
            Modifier
                .fillMaxSize()
                .background(palette.bg)
                .safeDrawingPadding(),
        ) {
            when (screen) {
                Screen.Home -> HomeScreen(
                    continueName = routines.continueName,
                    onContinue = { routines.get(routines.continueName)?.let(::startRoutine) },
                    onChooseRoutine = { screen = Screen.ChooseRoutine },
                    onFocus = ::enterFocus,
                    onSettings = { screen = Screen.Settings },
                )
                Screen.Focus -> FocusScreen(state, send, onOptions = { sheet = Sheet.Options })
                Screen.Settings -> SettingsScreen(
                    onBack = { screen = Screen.Home },
                    onSounds = { screen = Screen.SoundsConfig },
                    onPresets = { screen = Screen.PresetsConfig },
                    onRoutines = { screen = Screen.RoutinesConfig },
                )
                Screen.SoundsConfig -> SoundsConfigScreen(state, onBack = { screen = Screen.Settings })
                Screen.PresetsConfig -> PresetsConfigScreen(
                    state, send,
                    onNew = {
                        PresetPreview.begin(state, send)
                        screen = Screen.PresetBuilder
                    },
                    onBack = { screen = Screen.Settings },
                )
                Screen.PresetBuilder -> PresetBuilderScreen(state, send, onLeave = ::leaveBuilder)
                Screen.RoutinesConfig -> RoutinesConfigScreen(
                    routines,
                    onEdit = {
                        editIndex = it
                        screen = Screen.RoutineEditor
                    },
                    onBack = { screen = Screen.Settings },
                )
                Screen.RoutineEditor -> RoutineEditorScreen(
                    routines, editIndex,
                    onClose = { screen = Screen.RoutinesConfig },
                )
                Screen.ChooseRoutine -> ChooseRoutineScreen(
                    routines.routines,
                    onStart = ::startRoutine,
                    onBack = { screen = Screen.Home },
                )
                Screen.RoutineRun -> {
                    val r = routines.get(runName)
                    if (r == null) {
                        LaunchedEffect(Unit) { leaveRun() }
                    } else {
                        RoutineRunScreen(
                            r,
                            onBack = ::leaveRun,
                            onToFocus = {
                                leaveRun()
                                enterFocus()
                            },
                        )
                    }
                }
            }
        }

        if (preStart) {
            PreStartDialog(
                initialMinutes = prefs.lastMinutes,
                presets = state.presets,
                onStart = { m, preset ->
                    prefs.lastMinutes = m
                    send(EngineCommand.Timer(m))
                    if (preset != null) send(EngineCommand.LoadPreset(preset))
                    preStart = false
                    screen = Screen.Focus
                },
                onNoTimer = {
                    preStart = false
                    screen = Screen.Focus
                },
                onDismiss = { preStart = false },
            )
        }

        when (sheet) {
            Sheet.None -> {}
            Sheet.Options -> OptionsSheet(
                state, send,
                onTimer = { sheet = Sheet.Timer },
                onLoadPreset = { sheet = Sheet.LoadPreset },
                onHome = {
                    sheet = Sheet.None
                    screen = Screen.Home
                },
                onDismiss = { sheet = Sheet.None },
            )
            Sheet.Timer -> TimerSheet(
                hasTimer = state.timer != null,
                lastMinutes = prefs.lastMinutes,
                send = send,
                onUsed = { prefs.lastMinutes = it },
                onBack = { sheet = Sheet.Options },
                onDismiss = { sheet = Sheet.None },
            )
            Sheet.LoadPreset -> LoadPresetSheet(
                presets = state.presets,
                onPick = {
                    send(EngineCommand.LoadPreset(it))
                    sheet = Sheet.None
                },
                onBack = { sheet = Sheet.Options },
                onDismiss = { sheet = Sheet.None },
            )
        }
    }
}
