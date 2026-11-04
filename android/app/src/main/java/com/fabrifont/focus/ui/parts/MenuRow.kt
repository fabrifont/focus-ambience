package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.theme.LocalPalette

private val RowShape = RoundedCornerShape(Dimens.rowRadius)

private fun Modifier.dashedBorder(color: Color) = drawBehind {
    drawRoundRect(
        color = color,
        cornerRadius = CornerRadius(Dimens.rowRadius.toPx()),
        style = Stroke(
            width = Dimens.rowBorder.toPx(),
            pathEffect = PathEffect.dashPathEffect(floatArrayOf(Dimens.dashOn, Dimens.dashOff)),
        ),
    )
}

/** Fila de menú estilo terminal; deshabilitada = borde punteado (y «próximamente» si `soon`). */
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
            .let {
                if (off) it.dashedBorder(p.dim.copy(alpha = Dimens.disabledAlpha))
                else it.border(Dimens.rowBorder, p.dim, RowShape)
            }
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = Dimens.rowPadH, vertical = Dimens.rowPadV),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.rowGap),
    ) {
        Text(
            label,
            color = if (off) p.dim else p.fg,
            fontSize = FontSizes.body,
            modifier = Modifier.weight(1f),
        )
        if (soon) Text(Txt.soon, color = p.dim, fontSize = FontSizes.small)
        if (chevron && enabled) Text("›", color = p.accent, fontSize = FontSizes.chevron)
    }
}
