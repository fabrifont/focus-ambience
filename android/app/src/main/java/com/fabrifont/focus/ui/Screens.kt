package com.fabrifont.focus.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.engine.SoundItem
import com.fabrifont.focus.ui.theme.LocalPalette
import com.fabrifont.focus.ui.theme.themeGlyph

@Composable
private fun ScreenColumn(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp, vertical = 16.dp),
        content = content,
    )
}

@Composable
private fun Title(text: String) {
    Text(
        text,
        color = LocalPalette.current.accent,
        fontSize = 22.sp,
        fontWeight = FontWeight.Bold,
        modifier = Modifier.padding(bottom = 20.dp),
    )
}

// ---------- 1 · Inicio ----------

@Composable
fun HomeScreen(onFocus: () -> Unit, onSettings: () -> Unit) {
    val p = LocalPalette.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            "focus",
            color = p.accent,
            fontSize = 72.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 36.dp),
            textAlign = TextAlign.Center,
        )
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            MenuRow("Continuar", enabled = false, soon = true, tag = "home-continue")
            MenuRow("Elegir rutina", enabled = false, soon = true, tag = "home-routines")
            MenuRow("Ir a Focus", tag = "home-focus", onClick = onFocus)
            MenuRow("Configuración", tag = "home-settings", onClick = onSettings)
        }
    }
}

// ---------- 3 · Focus ----------

@Composable
fun FocusScreen(state: EngineState, send: (EngineCommand) -> Unit, onOptions: () -> Unit) {
    val p = LocalPalette.current
    ScreenColumn {
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Text(
                "⋯ Opciones",
                color = p.accent,
                fontSize = 16.sp,
                modifier = Modifier
                    .testTag("options")
                    .clickable(onClick = onOptions)
                    .padding(8.dp),
            )
        }
        Clock(state, onToggle = { send(EngineCommand.ToggleTimerPause) })
        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            items(state.sounds, key = { it.id }) { s ->
                SoundRow(
                    s,
                    on = s.id in state.active && !state.paused,
                    volume = state.volumes[s.id] ?: 0.5f,
                    send = send,
                )
            }
        }
    }
}

@Composable
private fun Clock(state: EngineState, onToggle: () -> Unit) {
    val p = LocalPalette.current
    val t = state.timer
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = if (t != null) formatClock(t.remainingSecs) else "--:--",
            color = if (t == null || t.paused) p.dim else p.fg,
            fontSize = 72.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier
                .testTag("clock")
                .clickable(enabled = t != null, onClick = onToggle),
        )
        Text(
            text = if (t?.paused == true) "timer en pausa · tocá para seguir" else " ",
            color = p.accent2,
            fontSize = 13.sp,
            modifier = Modifier.testTag("clock-hint"),
        )
    }
}

@Composable
private fun SoundRow(s: SoundItem, on: Boolean, volume: Float, send: (EngineCommand) -> Unit) {
    val p = LocalPalette.current
    Column(modifier = Modifier.testTag("sound-${s.id}")) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(themeGlyph(s.theme), color = p.accent, fontSize = 20.sp)
            Text(s.name, color = if (on) p.fg else p.dim, fontSize = 16.sp, modifier = Modifier.weight(1f))
            Switch(
                checked = on,
                onCheckedChange = { send(EngineCommand.Toggle(s.id)) },
                colors = SwitchDefaults.colors(
                    checkedThumbColor = p.bg,
                    checkedTrackColor = p.accent,
                    uncheckedThumbColor = p.dim,
                    uncheckedTrackColor = p.surface,
                    uncheckedBorderColor = p.dim,
                ),
                modifier = Modifier.testTag("switch-${s.id}"),
            )
        }
        VolumeSlider(
            value = volume,
            onChange = { send(EngineCommand.SetVolume(s.id, it)) },
            tag = "volume-${s.id}",
        )
    }
}

// ---------- 6 · Configuración ----------

@Composable
fun SettingsScreen(onSounds: () -> Unit) {
    ScreenColumn {
        Title("Configuración")
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            MenuRow("Sonidos ambientales", chevron = true, tag = "settings-sounds", onClick = onSounds)
            MenuRow("Presets", enabled = false, soon = true)
            MenuRow("Rutinas", enabled = false, soon = true)
            MenuRow("Meditaciones", enabled = false, soon = true)
        }
    }
}

// ---------- 7 · Configuración › Sonidos ambientales ----------

@Composable
fun SoundsConfigScreen(state: EngineState) {
    val p = LocalPalette.current
    var picked by rememberSaveable { mutableStateOf<String?>(null) }
    ScreenColumn {
        Title("Sonidos ambientales")
        LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            items(state.sounds, key = { it.id }) { s ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag("config-${s.id}")
                        .clickable { picked = s.id }
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(themeGlyph(s.theme), color = p.accent, fontSize = 20.sp)
                    Text(s.name, color = p.fg, fontSize = 16.sp, modifier = Modifier.weight(1f))
                    Text(s.theme, color = p.dim, fontSize = 12.sp)
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        MenuRow("+ Agregar desde YouTube", enabled = false, soon = true, tag = "config-add")
    }
    if (picked != null) {
        AlertDialog(
            onDismissRequest = { picked = null },
            containerColor = p.surface,
            titleContentColor = p.fg,
            textContentColor = p.fg,
            text = { Text("Los sonidos generados no se pueden borrar") },
            confirmButton = {
                TextButton(onClick = { picked = null }, modifier = Modifier.testTag("dialog-back")) {
                    Text("Volver", color = p.accent)
                }
            },
            modifier = Modifier.testTag("delete-dialog"),
        )
    }
}
