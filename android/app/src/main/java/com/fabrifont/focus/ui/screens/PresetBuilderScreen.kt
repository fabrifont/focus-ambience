package com.fabrifont.focus.ui.screens

import androidx.activity.compose.BackHandler
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
import com.fabrifont.focus.ui.parts.BottomButton
import com.fabrifont.focus.ui.parts.Choice
import com.fabrifont.focus.ui.parts.ChoiceDialog
import com.fabrifont.focus.ui.parts.Hint
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle
import com.fabrifont.focus.ui.parts.SoundMixRow
import com.fabrifont.focus.ui.parts.TextInputDialog

/**
 * Nuevo preset: los sonidos suenan en vivo (el motor ya está silenciado por [com.fabrifont.focus.ui.PresetPreview]).
 * `onLeave` sale y restaura el audio de antes; se llama al guardar o al descartar.
 */
@Composable
fun PresetBuilderScreen(state: EngineState, send: (EngineCommand) -> Unit, onLeave: () -> Unit) {
    var asking by rememberSaveable { mutableStateOf(false) }
    var discarding by rememberSaveable { mutableStateOf(false) }
    var nameError by rememberSaveable { mutableStateOf<String?>(null) }
    val chosen = state.sounds.filter { it.id in state.active }
    val dirty = chosen.isNotEmpty()

    fun back() {
        if (dirty) discarding = true else onLeave()
    }
    BackHandler(onBack = ::back)

    ScreenColumn {
        ScreenTitle(Txt.builderTitle, onBack = ::back)
        Hint(Txt.builderHint)
        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = Dimens.gap),
        ) {
            items(state.sounds, key = { it.id }) { s ->
                SoundMixRow(
                    s,
                    on = s.id in state.active,
                    volume = state.volumeOf(s.id),
                    onToggle = { send(EngineCommand.SetActive(s.id, s.id !in state.active)) },
                    onVolume = { send(EngineCommand.SetVolume(s.id, it)) },
                )
            }
        }
        BottomButton(Txt.builderSave, tag = "builder-save", enabled = dirty) { asking = true }
    }

    if (asking) {
        TextInputDialog(
            title = Txt.builderNameTitle,
            initial = "",
            confirmLabel = Txt.builderNameSave,
            error = nameError,
            onDismiss = {
                asking = false
                nameError = null
            },
            onConfirm = { name ->
                when {
                    name.isBlank() -> nameError = Txt.builderNameEmpty
                    state.presets.any { it.name == name } -> nameError = Txt.builderNameTaken
                    else -> {
                        send(EngineCommand.StorePreset(name, chosen.associate { it.id to state.volumeOf(it.id) }))
                        asking = false
                        nameError = null
                        onLeave()
                    }
                }
            },
        )
    }
    if (discarding) {
        ChoiceDialog(
            title = Txt.builderDiscardTitle,
            choices = listOf(
                Choice(Txt.builderDiscardYes, "discard-yes") {
                    discarding = false
                    onLeave()
                },
                Choice(Txt.builderKeep, "discard-keep", marked = true) { discarding = false },
            ),
            onDismiss = { discarding = false },
            tag = "discard-dialog",
        )
    }
}
