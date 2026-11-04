package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.theme.LocalPalette

private const val MAX_MINUTES = 999

/** −5 / campo numérico / +5. Avisa solo valores válidos (1..999). */
@Composable
fun MinutesEditor(minutes: Int, onChange: (Int) -> Unit, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    var text by remember { mutableStateOf(minutes.toString()) }
    fun step(delta: Int) {
        val v = ((text.toIntOrNull() ?: minutes) + delta).coerceIn(1, MAX_MINUTES)
        text = v.toString()
        onChange(v)
    }
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(Dimens.gap, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OutlinedButton(onClick = { step(-5) }, modifier = Modifier.testTag("minutes-minus")) {
            Text(Txt.minus5, color = p.accent)
        }
        OutlinedTextField(
            value = text,
            onValueChange = { t ->
                val d = t.filter { it.isDigit() }.take(3)
                text = d
                d.toIntOrNull()?.let { onChange(it.coerceIn(1, MAX_MINUTES)) }
            },
            singleLine = true,
            suffix = { Text(Txt.minutesSuffix, color = p.dim) },
            textStyle = TextStyle(fontSize = FontSizes.input, textAlign = TextAlign.Center, color = p.fg),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier
                .width(Dimens.fieldWidth)
                .testTag("minutes-field"),
        )
        OutlinedButton(onClick = { step(5) }, modifier = Modifier.testTag("minutes-plus")) {
            Text(Txt.plus5, color = p.accent)
        }
    }
}
