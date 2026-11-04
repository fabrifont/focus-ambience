package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.ConfirmDialog
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle

/** Configuración › Presets. Tocar uno pregunta si borrarlo (es la única acción). */
@Composable
fun PresetsConfigScreen(
    state: EngineState,
    send: (EngineCommand) -> Unit,
    onNew: () -> Unit,
    onBack: () -> Unit,
) {
    var toDelete by rememberSaveable { mutableStateOf<String?>(null) }
    ScreenColumn {
        ScreenTitle(Txt.presetsTitle, onBack)
        MenuRow(Txt.presetsNew, tag = "presets-new", onClick = onNew)
        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(top = Dimens.gap),
        ) {
            items(state.presets, key = { it.name }) { preset ->
                ListRow(
                    preset.name,
                    tag = "preset-${preset.name}",
                    subtitle = Txt.presetSounds(preset.layers.size),
                ) { toDelete = preset.name }
            }
        }
    }
    toDelete?.let { name ->
        ConfirmDialog(
            title = Txt.deletePresetTitle(name),
            yesLabel = Txt.deleteYes,
            onYes = {
                send(EngineCommand.DeletePreset(name))
                toDelete = null
            },
            onNo = { toDelete = null },
        )
    }
}
