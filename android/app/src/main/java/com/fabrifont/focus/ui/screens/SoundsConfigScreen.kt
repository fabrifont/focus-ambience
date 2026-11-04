package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle
import com.fabrifont.focus.ui.theme.LocalPalette
import com.fabrifont.focus.ui.theme.themeGlyph

@Composable
fun SoundsConfigScreen(state: EngineState, onBack: () -> Unit) {
    val p = LocalPalette.current
    var picked by rememberSaveable { mutableStateOf<String?>(null) }
    ScreenColumn {
        ScreenTitle(Txt.soundsTitle, onBack)
        LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(Dimens.gap)) {
            items(state.sounds, key = { it.id }) { s ->
                ListRow(s.name, tag = "config-${s.id}", leading = themeGlyph(s.theme), trailing = s.theme) {
                    picked = s.id
                }
            }
        }
        Spacer(Modifier.height(Dimens.bottomBarTop))
        MenuRow(Txt.soundsAdd, enabled = false, soon = true, tag = "config-add")
    }
    if (picked != null) {
        AlertDialog(
            onDismissRequest = { picked = null },
            containerColor = p.surface,
            titleContentColor = p.fg,
            textContentColor = p.fg,
            text = { Text(Txt.soundsNoDelete) },
            confirmButton = {
                TextButton(onClick = { picked = null }, modifier = Modifier.testTag("dialog-back")) {
                    Text(Txt.soundsBack, color = p.accent)
                }
            },
            modifier = Modifier.testTag("delete-dialog"),
        )
    }
}
