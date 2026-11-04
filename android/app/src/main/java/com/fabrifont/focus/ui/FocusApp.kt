package com.fabrifont.focus.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.Composable
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
import com.fabrifont.focus.ui.theme.FocusTheme
import com.fabrifont.focus.ui.theme.GeneratedPalette
import com.fabrifont.focus.ui.theme.IdlePalette

private enum class Screen { Home, Focus, Settings, SoundsConfig }
private enum class Sheet { None, Options, Timer }

@Composable
fun FocusApp(engine: FocusEngine, prefs: Prefs) {
    val state by engine.state.collectAsState()

    // Etapa 1: todo lo que suena es tema Generado; sin sonido, el crepúsculo neutro.
    val mix by animateFloatAsState(if (state.audible) 1f else 0f, tween(900), label = "palette")
    val palette = remember(mix) { IdlePalette.lerp(GeneratedPalette, mix) }

    var screen by rememberSaveable { mutableStateOf(Screen.Home) }
    var sheet by rememberSaveable { mutableStateOf(Sheet.None) }
    var preStart by rememberSaveable { mutableStateOf(false) }
    val send = remember(engine) { { cmd: EngineCommand -> engine.send(cmd) } }

    BackHandler(enabled = screen != Screen.Home) {
        screen = when (screen) {
            Screen.SoundsConfig -> Screen.Settings
            else -> Screen.Home
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
                    onFocus = {
                        // Con una sesión en curso (algo prendido o timer) se entra directo.
                        if (state.busy) screen = Screen.Focus else preStart = true
                    },
                    onSettings = { screen = Screen.Settings },
                )
                Screen.Focus -> FocusScreen(state, send, onOptions = { sheet = Sheet.Options })
                Screen.Settings -> SettingsScreen(onSounds = { screen = Screen.SoundsConfig })
                Screen.SoundsConfig -> SoundsConfigScreen(state)
            }
        }

        if (preStart) {
            PreStartDialog(
                initialMinutes = prefs.lastMinutes,
                onStart = { m ->
                    prefs.lastMinutes = m
                    send(EngineCommand.Timer(m))
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
        }
    }
}
