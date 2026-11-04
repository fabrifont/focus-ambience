package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.formatClock
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.SoundMixRow
import com.fabrifont.focus.ui.theme.LocalPalette

@Composable
fun FocusScreen(state: EngineState, send: (EngineCommand) -> Unit, onOptions: () -> Unit) {
    val p = LocalPalette.current
    ScreenColumn {
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Text(
                Txt.focusOptions,
                color = p.accent,
                fontSize = FontSizes.body,
                modifier = Modifier
                    .testTag("options")
                    .clickable(onClick = onOptions)
                    .padding(Dimens.tapPad),
            )
        }
        Clock(state, onToggle = { send(EngineCommand.ToggleTimerPause) })
        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall),
        ) {
            items(state.sounds, key = { it.id }) { s ->
                SoundMixRow(
                    s,
                    on = s.id in state.active && !state.paused,
                    volume = state.volumeOf(s.id),
                    onToggle = { send(EngineCommand.Toggle(s.id)) },
                    onVolume = { send(EngineCommand.SetVolume(s.id, it)) },
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
            .padding(vertical = Dimens.clockV),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = if (t != null) formatClock(t.remainingSecs) else Txt.clockEmpty,
            color = if (t == null || t.paused) p.dim else p.fg,
            fontSize = FontSizes.clock,
            fontWeight = FontWeight.Bold,
            modifier = Modifier
                .testTag("clock")
                .clickable(enabled = t != null, onClick = onToggle),
        )
        Text(
            text = if (t?.paused == true) Txt.clockPausedHint else " ",
            color = p.accent2,
            fontSize = FontSizes.hint,
            modifier = Modifier.testTag("clock-hint"),
        )
    }
}
