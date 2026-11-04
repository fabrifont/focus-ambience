package com.fabrifont.focus.ui

import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Espaciados y formas. Las pantallas no llevan números sueltos: se cambian acá. */
object Dimens {
    val screenH = 20.dp
    val screenV = 16.dp
    val homeH = 24.dp

    val gap = 10.dp
    val gapSmall = 6.dp
    val gapLarge = 14.dp
    val rowGap = 12.dp
    val titleBottom = 20.dp
    val logoBottom = 36.dp
    val clockV = 20.dp
    val bottomBarTop = 8.dp

    val rowRadius = 6.dp
    val rowBorder = 1.dp
    val rowPadH = 16.dp
    val rowPadV = 14.dp
    val listRowPadV = 8.dp
    val tapPad = 8.dp

    val sheetH = 20.dp
    val sheetV = 8.dp

    val fieldWidth = 140.dp

    /** Largo de raya y de hueco del borde punteado de las filas deshabilitadas. */
    val dashOn = 10f
    val dashOff = 8f
    const val disabledAlpha = 0.7f
    const val sliderTrackAlpha = 0.4f
    const val movingAlpha = 0.18f
}

/** Tamaños de letra. */
object FontSizes {
    val logo = 72.sp
    val clock = 72.sp
    val title = 22.sp
    val dialogTitle = 18.sp
    val input = 20.sp
    val glyph = 20.sp
    val chevron = 18.sp
    val body = 16.sp
    val hint = 13.sp
    val small = 12.sp
}
