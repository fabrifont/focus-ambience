package com.fabrifont.focus.ui

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.fabrifont.focus.ui.theme.LocalPalette

fun formatClock(secs: Long): String {
    val s = secs.coerceAtLeast(0)
    val h = s / 3600
    val m = s % 3600 / 60
    val ss = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, ss) else "%02d:%02d".format(m, ss)
}

private val RowShape = RoundedCornerShape(6.dp)

private fun Modifier.dashedBorder(color: Color) = drawBehind {
    drawRoundRect(
        color = color,
        cornerRadius = CornerRadius(6.dp.toPx()),
        style = Stroke(width = 1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f))),
    )
}

/** Fila de menú estilo terminal; deshabilitada = borde punteado y etiqueta «próximamente». */
@Composable
fun MenuRow(
    label: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    soon: Boolean = false,
    chevron: Boolean = false,
    tag: String? = null,
    onClick: () -> Unit = {},
) {
    val p = LocalPalette.current
    val off = !enabled
    Row(
        modifier = modifier
            .fillMaxWidth()
            .let { if (tag != null) it.testTag(tag) else it }
            .clip(RowShape)
            .let { if (off) it.dashedBorder(p.dim.copy(alpha = 0.7f)) else it.border(1.dp, p.dim, RowShape) }
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            label,
            color = if (off) p.dim else p.fg,
            fontSize = 16.sp,
            modifier = Modifier.weight(1f),
        )
        if (soon) Text("próximamente", color = p.dim, fontSize = 12.sp)
        if (chevron && enabled) Text("›", color = p.accent, fontSize = 18.sp)
    }
}

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
            inactiveTrackColor = p.dim.copy(alpha = 0.4f),
        ),
        modifier = modifier
            .fillMaxWidth()
            .let { if (tag != null) it.testTag(tag) else it },
    )
}
