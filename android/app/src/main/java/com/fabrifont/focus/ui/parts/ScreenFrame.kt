package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.theme.LocalPalette

@Composable
fun ScreenColumn(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = Dimens.screenH, vertical = Dimens.screenV),
        content = content,
    )
}

/** «‹ Título»: tocar vuelve (igual que el gesto atrás). */
@Composable
fun ScreenTitle(text: String, onBack: () -> Unit) {
    val p = LocalPalette.current
    Text(
        "${Txt.back} $text",
        color = p.accent,
        fontSize = FontSizes.title,
        fontWeight = FontWeight.Bold,
        modifier = Modifier
            .testTag("back")
            .clickable(onClick = onBack)
            .padding(bottom = Dimens.titleBottom),
    )
}

/** Texto chico de ayuda o aviso. */
@Composable
fun Hint(text: String, modifier: Modifier = Modifier, tag: String? = null) {
    val p = LocalPalette.current
    Text(
        text,
        color = p.dim,
        fontSize = FontSizes.hint,
        modifier = modifier.let { if (tag != null) it.testTag(tag) else it },
    )
}
