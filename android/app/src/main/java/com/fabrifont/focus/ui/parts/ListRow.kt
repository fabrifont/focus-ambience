package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.theme.LocalPalette

/** Fila de lista sin borde: glifo opcional, título, subtítulo y texto a la derecha. */
@Composable
fun ListRow(
    title: String,
    modifier: Modifier = Modifier,
    tag: String? = null,
    leading: String? = null,
    subtitle: String? = null,
    trailing: String? = null,
    highlight: Boolean = false,
    onClick: () -> Unit,
) {
    val p = LocalPalette.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .let { if (tag != null) it.testTag(tag) else it }
            .let { if (highlight) it.background(p.accent.copy(alpha = Dimens.movingAlpha)) else it }
            .clickable(onClick = onClick)
            .padding(vertical = Dimens.listRowPadV),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.rowGap),
    ) {
        if (leading != null) Text(leading, color = p.accent, fontSize = FontSizes.glyph)
        Column(modifier = Modifier.weight(1f)) {
            Text(title, color = p.fg, fontSize = FontSizes.body)
            if (subtitle != null) Text(subtitle, color = p.dim, fontSize = FontSizes.small)
        }
        if (trailing != null) Text(trailing, color = p.dim, fontSize = FontSizes.small)
    }
}
