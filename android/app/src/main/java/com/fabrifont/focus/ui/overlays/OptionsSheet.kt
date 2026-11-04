package com.fabrifont.focus.ui.overlays

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Layout
import com.fabrifont.focus.ui.OptionsEntry
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.SheetScaffold
import com.fabrifont.focus.ui.parts.VolumeSlider
import com.fabrifont.focus.ui.theme.LocalPalette

/** Hoja «Opciones» de Focus: solo usa o elige, nada que cree o borre. */
@Composable
fun OptionsSheet(
    state: EngineState,
    send: (EngineCommand) -> Unit,
    onTimer: () -> Unit,
    onLoadPreset: () -> Unit,
    onHome: () -> Unit,
    onDismiss: () -> Unit,
) {
    val p = LocalPalette.current
    SheetScaffold("options-sheet", onDismiss) {
        for (entry in Layout.options) when (entry) {
            OptionsEntry.Timer -> MenuRow(Txt.optTimer, chevron = true, tag = "opt-timer", onClick = onTimer)
            OptionsEntry.LoadPreset ->
                MenuRow(Txt.optLoadPreset, chevron = true, tag = "opt-preset", onClick = onLoadPreset)
            OptionsEntry.Master -> Column(modifier = Modifier.testTag("opt-master")) {
                Text(
                    Txt.optMaster,
                    color = p.fg,
                    fontSize = FontSizes.body,
                    modifier = Modifier.padding(start = Dimens.gapSmall, top = Dimens.gapSmall),
                )
                VolumeSlider(state.master, onChange = { send(EngineCommand.SetMaster(it)) }, tag = "master-slider")
            }
            OptionsEntry.Pause -> {
                val resume = !state.running && state.busy
                MenuRow(if (resume) Txt.optResume else Txt.optPause, enabled = state.busy, tag = "opt-pause") {
                    send(if (resume) EngineCommand.ResumeBoth else EngineCommand.PauseBoth)
                    onDismiss()
                }
            }
            OptionsEntry.Stop -> MenuRow(Txt.optStop, tag = "opt-stop") {
                send(EngineCommand.StopAll)
                onDismiss()
            }
            OptionsEntry.Home -> MenuRow(Txt.optHome, tag = "opt-home", onClick = onHome)
        }
    }
}
