package com.fabrifont.focus.ui.overlays

import androidx.compose.runtime.Composable
import com.fabrifont.focus.engine.PresetItem
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.Hint
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.SheetScaffold

/** Opciones › Cargar preset: tocar uno lo carga y cierra. Desde acá no se crea ni se borra nada. */
@Composable
fun LoadPresetSheet(
    presets: List<PresetItem>,
    onPick: (String) -> Unit,
    onBack: () -> Unit,
    onDismiss: () -> Unit,
) {
    SheetScaffold("load-preset-sheet", onDismiss) {
        MenuRow(Txt.loadPresetTitle, tag = "load-preset-back", onClick = onBack)
        if (presets.isEmpty()) Hint(Txt.presetsEmpty)
        for (p in presets) MenuRow(p.name, tag = "load-preset-${p.name}") { onPick(p.name) }
    }
}
