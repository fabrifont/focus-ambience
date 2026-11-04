package com.fabrifont.focus.ui.overlays

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.ui.Layout
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.MinutesEditor
import com.fabrifont.focus.ui.parts.SheetScaffold
import com.fabrifont.focus.ui.theme.LocalPalette

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
    var minutes by remember { mutableIntStateOf(lastMinutes) }
    fun start(m: Int) {
        onUsed(m)
        send(EngineCommand.Timer(m))
        onDismiss()
    }
    SheetScaffold("timer-sheet", onDismiss) {
        MenuRow(Txt.timerBack, tag = "timer-back", onClick = onBack)
        for (m in Layout.timerShortcuts) MenuRow(Txt.timerMinutes(m), tag = "timer-$m") { start(m) }
        MenuRow(Txt.timerCustom, chevron = !custom, tag = "timer-custom") { custom = !custom }
        if (custom) {
            MinutesEditor(minutes, onChange = { minutes = it })
            OutlinedButton(
                onClick = { start(minutes) },
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("timer-custom-go"),
            ) { Text(Txt.timerStart(minutes), color = p.accent) }
        }
        MenuRow(Txt.timerCancel, enabled = hasTimer, tag = "timer-cancel") {
            send(EngineCommand.Timer(null))
            onDismiss()
        }
    }
}
