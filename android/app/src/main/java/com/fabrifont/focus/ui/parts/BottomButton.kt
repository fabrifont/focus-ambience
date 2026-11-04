package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.theme.LocalPalette

/** Botón ancho, fijo al pie de la pantalla (va después del contenido que pesa). */
@Composable
fun BottomButton(label: String, tag: String, modifier: Modifier = Modifier, enabled: Boolean = true, onClick: () -> Unit) {
    val p = LocalPalette.current
    Button(
        onClick = onClick,
        enabled = enabled,
        colors = ButtonDefaults.buttonColors(
            containerColor = p.accent,
            contentColor = p.bg,
            disabledContainerColor = p.surface,
            disabledContentColor = p.dim,
        ),
        modifier = modifier
            .fillMaxWidth()
            .padding(top = Dimens.bottomBarTop)
            .testTag(tag),
    ) { Text(label, fontSize = FontSizes.body) }
}
