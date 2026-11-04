package com.fabrifont.focus.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily

/** Paleta por tema, copiada de `src/tui/theme.rs` del escritorio. */
@Immutable
data class Palette(
    val bg: Color,
    /** Superficie apenas levantada: filas elegidas, diálogos. */
    val surface: Color,
    val fg: Color,
    val dim: Color,
    val accent: Color,
    val accent2: Color,
) {
    fun lerp(o: Palette, t: Float) = Palette(
        lerp(bg, o.bg, t), lerp(surface, o.surface, t), lerp(fg, o.fg, t),
        lerp(dim, o.dim, t), lerp(accent, o.accent, t), lerp(accent2, o.accent2, t),
    )
}

/** Cuando no suena nada: un crepúsculo tranquilo. */
val IdlePalette = Palette(
    bg = Color(0xFF0C0D12),
    surface = Color(0xFF171922),
    fg = Color(0xFFD8DAE6),
    dim = Color(0xFF585C70),
    accent = Color(0xFF9AA6FF),
    accent2 = Color(0xFFC3A6FF),
)

val GeneratedPalette = Palette(
    bg = Color(0xFF0E0B15),
    surface = Color(0xFF1B1626),
    fg = Color(0xFFE6E0F5),
    dim = Color(0xFF5F5778),
    accent = Color(0xFFB69CFF),
    accent2 = Color(0xFFFF9CC8),
)

val LocalPalette = staticCompositionLocalOf { IdlePalette }

fun themeGlyph(theme: String): String = when (theme) {
    "Generados" -> "∿"
    else -> "♪"
}

private val Mono = FontFamily.Monospace

private fun mono(s: TextStyle) = s.copy(fontFamily = Mono)

private val MonoTypography = Typography().let {
    it.copy(
        displayLarge = mono(it.displayLarge), displayMedium = mono(it.displayMedium),
        displaySmall = mono(it.displaySmall), headlineLarge = mono(it.headlineLarge),
        headlineMedium = mono(it.headlineMedium), headlineSmall = mono(it.headlineSmall),
        titleLarge = mono(it.titleLarge), titleMedium = mono(it.titleMedium),
        titleSmall = mono(it.titleSmall), bodyLarge = mono(it.bodyLarge),
        bodyMedium = mono(it.bodyMedium), bodySmall = mono(it.bodySmall),
        labelLarge = mono(it.labelLarge), labelMedium = mono(it.labelMedium),
        labelSmall = mono(it.labelSmall),
    )
}

@Composable
fun FocusTheme(palette: Palette, content: @Composable () -> Unit) {
    val scheme = darkColorScheme(
        primary = palette.accent,
        onPrimary = palette.bg,
        secondary = palette.accent2,
        onSecondary = palette.bg,
        background = palette.bg,
        onBackground = palette.fg,
        surface = palette.surface,
        onSurface = palette.fg,
        surfaceVariant = palette.surface,
        onSurfaceVariant = palette.dim,
        outline = palette.dim,
        outlineVariant = palette.dim,
    )
    androidx.compose.runtime.CompositionLocalProvider(LocalPalette provides palette) {
        MaterialTheme(colorScheme = scheme, typography = MonoTypography, content = content)
    }
}
