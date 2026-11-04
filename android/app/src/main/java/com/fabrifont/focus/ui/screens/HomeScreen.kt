package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.HomeEntry
import com.fabrifont.focus.ui.Layout
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.theme.LocalPalette

/** Inicio. `continueName` = la última rutina, solo si todavía existe. Acá se usa, no se edita. */
@Composable
fun HomeScreen(
    continueName: String?,
    onContinue: () -> Unit,
    onChooseRoutine: () -> Unit,
    onFocus: () -> Unit,
    onSettings: () -> Unit,
) {
    val p = LocalPalette.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = Dimens.homeH),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            Txt.appName,
            color = p.accent,
            fontSize = FontSizes.logo,
            fontWeight = FontWeight.Bold,
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = Dimens.logoBottom),
            textAlign = TextAlign.Center,
        )
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.gap)) {
            for (entry in Layout.home) when (entry) {
                HomeEntry.Continue -> if (continueName != null) {
                    MenuRow(Txt.homeContinueRoutine(continueName), tag = "home-continue", onClick = onContinue)
                }
                HomeEntry.ChooseRoutine ->
                    MenuRow(Txt.homeChooseRoutine, tag = "home-routines", onClick = onChooseRoutine)
                HomeEntry.Focus -> MenuRow(Txt.homeFocus, tag = "home-focus", onClick = onFocus)
                HomeEntry.Settings -> MenuRow(Txt.homeSettings, tag = "home-settings", onClick = onSettings)
            }
        }
    }
}
