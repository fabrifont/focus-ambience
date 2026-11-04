package com.fabrifont.focus.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.ui.theme.LocalPalette

private const val MAX_MINUTES = 999

/** −5 / campo numérico / +5. Avisa solo valores válidos (1..999). */
@Composable
fun MinutesEditor(minutes: Int, onChange: (Int) -> Unit, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    var text by remember { mutableStateOf(minutes.toString()) }
    fun step(delta: Int) {
        val v = ((text.toIntOrNull() ?: minutes) + delta).coerceIn(1, MAX_MINUTES)
        text = v.toString()
        onChange(v)
    }
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OutlinedButton(onClick = { step(-5) }, modifier = Modifier.testTag("minutes-minus")) {
            Text("−5", color = p.accent)
        }
        OutlinedTextField(
            value = text,
            onValueChange = { t ->
                val d = t.filter { it.isDigit() }.take(3)
                text = d
                d.toIntOrNull()?.let { onChange(it.coerceIn(1, MAX_MINUTES)) }
            },
            singleLine = true,
            suffix = { Text("min", color = p.dim) },
            textStyle = androidx.compose.ui.text.TextStyle(fontSize = 20.sp, textAlign = TextAlign.Center, color = p.fg),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier
                .width(140.dp)
                .testTag("minutes-field"),
        )
        OutlinedButton(onClick = { step(5) }, modifier = Modifier.testTag("minutes-plus")) {
            Text("+5", color = p.accent)
        }
    }
}

// ---------- 2 · Antes de arrancar ----------

@Composable
fun PreStartDialog(
    initialMinutes: Int,
    onStart: (Int) -> Unit,
    onNoTimer: () -> Unit,
    onDismiss: () -> Unit,
) {
    val p = LocalPalette.current
    var minutes by remember { mutableStateOf(initialMinutes) }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.surface,
        title = { Text("Antes de arrancar", color = p.accent, fontSize = 18.sp) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Text("¿De cuántos minutos?", color = p.fg)
                MinutesEditor(minutes, onChange = { minutes = it })
            }
        },
        confirmButton = {
            TextButton(onClick = { onStart(minutes) }, modifier = Modifier.testTag("prestart-go")) {
                Text("Seguir", color = p.accent)
            }
        },
        dismissButton = {
            TextButton(onClick = onNoTimer, modifier = Modifier.testTag("prestart-notimer")) {
                Text("Entrar sin timer", color = p.fg)
            }
        },
        modifier = Modifier.testTag("prestart"),
    )
}

// ---------- 4 · Opciones ----------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OptionsSheet(
    state: EngineState,
    send: (EngineCommand) -> Unit,
    onTimer: () -> Unit,
    onHome: () -> Unit,
    onDismiss: () -> Unit,
) {
    val p = LocalPalette.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = p.surface,
        modifier = Modifier.testTag("options-sheet"),
    ) {
        Column(
            modifier = Modifier
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            MenuRow("Timer", chevron = true, tag = "opt-timer", onClick = onTimer)
            MenuRow("Cargar preset", enabled = false, soon = true, tag = "opt-preset")
            Column(modifier = Modifier.testTag("opt-master")) {
                Text("Volumen general", color = p.fg, fontSize = 16.sp, modifier = Modifier.padding(start = 4.dp, top = 6.dp))
                VolumeSlider(state.master, onChange = { send(EngineCommand.SetMaster(it)) }, tag = "master-slider")
            }
            val resume = !state.running && state.busy
            MenuRow(if (resume) "Seguir todo" else "Pausar todo", enabled = state.busy, tag = "opt-pause") {
                send(if (resume) EngineCommand.ResumeBoth else EngineCommand.PauseBoth)
                onDismiss()
            }
            MenuRow("Apagar todo", tag = "opt-stop") {
                send(EngineCommand.StopAll)
                onDismiss()
            }
            MenuRow("Volver al inicio", tag = "opt-home", onClick = onHome)
        }
    }
}

// ---------- 5 · Timer ----------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TimerSheet(
    hasTimer: Boolean,
    lastMinutes: Int,
    send: (EngineCommand) -> Unit,
    onUsed: (Int) -> Unit,
    onBack: () -> Unit,
    onDismiss: () -> Unit,
) {
    val p = LocalPalette.current
    var custom by remember { mutableStateOf(false) }
    var minutes by remember { mutableStateOf(lastMinutes) }
    fun start(m: Int) {
        onUsed(m)
        send(EngineCommand.Timer(m))
        onDismiss()
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = p.surface,
        modifier = Modifier.testTag("timer-sheet"),
    ) {
        Column(
            modifier = Modifier
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            MenuRow("‹ Volver", tag = "timer-back", onClick = onBack)
            for (m in listOf(25, 50, 90)) MenuRow("$m min", tag = "timer-$m") { start(m) }
            MenuRow("Personalizado…", chevron = !custom, tag = "timer-custom") { custom = !custom }
            if (custom) {
                MinutesEditor(minutes, onChange = { minutes = it })
                OutlinedButton(
                    onClick = { start(minutes) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag("timer-custom-go"),
                ) { Text("Empezar $minutes min", color = p.accent) }
            }
            MenuRow("Cancelar timer", enabled = hasTimer, tag = "timer-cancel") {
                send(EngineCommand.Timer(null))
                onDismiss()
            }
        }
    }
}
