package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.SoundItem
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.theme.LocalPalette
import com.fabrifont.focus.ui.theme.themeGlyph

/** Sonido con interruptor y volumen: lo usan Focus y el armado de presets. */
@Composable
fun SoundMixRow(
    sound: SoundItem,
    on: Boolean,
    volume: Float,
    onToggle: () -> Unit,
    onVolume: (Float) -> Unit,
) {
    val p = LocalPalette.current
    Column(modifier = Modifier.testTag("sound-${sound.id}")) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Dimens.rowGap),
        ) {
            Text(themeGlyph(sound.theme), color = p.accent, fontSize = FontSizes.glyph)
            Text(
                sound.name,
                color = if (on) p.fg else p.dim,
                fontSize = FontSizes.body,
                modifier = Modifier.weight(1f),
            )
            Switch(
                checked = on,
                onCheckedChange = { onToggle() },
                colors = SwitchDefaults.colors(
                    checkedThumbColor = p.bg,
                    checkedTrackColor = p.accent,
                    uncheckedThumbColor = p.dim,
                    uncheckedTrackColor = p.surface,
                    uncheckedBorderColor = p.dim,
                ),
                modifier = Modifier.testTag("switch-${sound.id}"),
            )
        }
        VolumeSlider(value = volume, onChange = onVolume, tag = "volume-${sound.id}")
    }
}
