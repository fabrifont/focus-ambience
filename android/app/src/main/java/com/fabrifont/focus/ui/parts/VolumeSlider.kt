package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.theme.LocalPalette

/** Slider que manda el valor en vivo pero como mucho cada 100 ms (cada envío guarda a disco). */
@Composable
fun VolumeSlider(
    value: Float,
    onChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    tag: String? = null,
) {
    val p = LocalPalette.current
    var local by remember { mutableFloatStateOf(value) }
    var dragging by remember { mutableStateOf(false) }
    var lastSent by remember { mutableLongStateOf(0L) }
    val shown = if (dragging) local else value
    Slider(
        value = shown,
        onValueChange = {
            local = it
            dragging = true
            val now = System.currentTimeMillis()
            if (now - lastSent >= 100) {
                lastSent = now
                onChange(it)
            }
        },
        onValueChangeFinished = {
            onChange(local)
            dragging = false
        },
        colors = SliderDefaults.colors(
            thumbColor = p.accent,
            activeTrackColor = p.accent,
            inactiveTrackColor = p.dim.copy(alpha = Dimens.sliderTrackAlpha),
        ),
        modifier = modifier
            .fillMaxWidth()
            .let { if (tag != null) it.testTag(tag) else it },
    )
}
