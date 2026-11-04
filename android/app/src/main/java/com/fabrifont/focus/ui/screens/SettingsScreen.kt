package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.Layout
import com.fabrifont.focus.ui.SettingsEntry
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle

@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    onSounds: () -> Unit,
    onPresets: () -> Unit,
    onRoutines: () -> Unit,
) {
    ScreenColumn {
        ScreenTitle(Txt.settingsTitle, onBack)
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.gap)) {
            for (entry in Layout.settings) when (entry) {
                SettingsEntry.Sounds -> MenuRow(Txt.settingsSounds, chevron = true, tag = "settings-sounds", onClick = onSounds)
                SettingsEntry.Presets -> MenuRow(Txt.settingsPresets, chevron = true, tag = "settings-presets", onClick = onPresets)
                SettingsEntry.Routines -> MenuRow(Txt.settingsRoutines, chevron = true, tag = "settings-routines", onClick = onRoutines)
                SettingsEntry.Meditations ->
                    MenuRow(Txt.settingsMeditations, enabled = false, soon = true, tag = "settings-meditations")
            }
        }
    }
}
