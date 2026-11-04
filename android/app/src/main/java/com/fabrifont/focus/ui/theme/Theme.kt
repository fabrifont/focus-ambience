package com.fabrifont.focus.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val FocusColors = darkColorScheme(
    background = Color(0xFF0B0D12),
    surface = Color(0xFF0B0D12),
    onBackground = Color(0xFFD7DAE0),
    onSurface = Color(0xFFD7DAE0),
)

@Composable
fun FocusTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = FocusColors, content = content)
}
