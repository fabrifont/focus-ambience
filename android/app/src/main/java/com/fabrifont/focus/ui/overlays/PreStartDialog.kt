package com.fabrifont.focus.ui.overlays

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.PresetItem
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.MinutesEditor
import com.fabrifont.focus.ui.theme.LocalPalette

/**
 * «Antes de arrancar»: 1) los minutos, 2) el preset (o «Sonidos personalizados»). Sin presets
 * guardados queda un solo paso. `onStart(minutos, preset)` con preset null = lo de antes.
 */
@Composable
fun PreStartDialog(
    initialMinutes: Int,
    presets: List<PresetItem>,
    onStart: (Int, String?) -> Unit,
    onNoTimer: () -> Unit,
    onDismiss: () -> Unit,
) {
    val p = LocalPalette.current
    var minutes by remember { mutableIntStateOf(initialMinutes) }
    var step by rememberSaveable { mutableIntStateOf(1) }
    var picked by rememberSaveable { mutableStateOf<String?>(null) }
    val steps = if (presets.isEmpty()) 1 else 2
    val onPresetStep = step == 2 && steps == 2

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.surface,
        title = {
            Text(
                if (steps == 1) Txt.preStartTitle else Txt.preStartTitleStep(step, steps),
                color = p.accent,
                fontSize = FontSizes.dialogTitle,
            )
        },
        text = {
            if (!onPresetStep) {
                Column(verticalArrangement = Arrangement.spacedBy(Dimens.gapLarge)) {
                    Text(Txt.preStartMinutes, color = p.fg)
                    MinutesEditor(minutes, onChange = { minutes = it })
                }
            } else {
                Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                    Text(Txt.preStartPickPreset, color = p.fg)
                    for (preset in presets) {
                        ListRow(
                            preset.name,
                            tag = "prestart-preset-${preset.name}",
                            leading = if (picked == preset.name) "●" else "○",
                            highlight = picked == preset.name,
                        ) { picked = preset.name }
                    }
                    ListRow(
                        Txt.preStartCustom,
                        tag = "prestart-custom",
                        leading = if (picked == null) "●" else "○",
                        highlight = picked == null,
                    ) { picked = null }
                }
            }
        },
        confirmButton = {
            if (!onPresetStep) {
                TextButton(
                    onClick = { if (steps == 1) onStart(minutes, null) else step = 2 },
                    modifier = Modifier.testTag("prestart-go"),
                ) { Text(Txt.preStartNext, color = p.accent) }
            } else {
                TextButton(onClick = { onStart(minutes, picked) }, modifier = Modifier.testTag("prestart-start")) {
                    Text(Txt.preStartGo, color = p.accent)
                }
            }
        },
        dismissButton = {
            if (!onPresetStep) {
                TextButton(onClick = onNoTimer, modifier = Modifier.testTag("prestart-notimer")) {
                    Text(Txt.preStartNoTimer, color = p.fg)
                }
            } else {
                TextButton(onClick = onNoTimer, modifier = Modifier.testTag("prestart-skip")) {
                    Text(Txt.preStartSkip, color = p.fg)
                }
            }
        },
        modifier = Modifier.testTag("prestart"),
    )
}
