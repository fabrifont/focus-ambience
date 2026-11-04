package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.theme.LocalPalette

/** Una opción de un diálogo; `marked` = la opción segura, resaltada (y la que hace `Esc`/atrás). */
data class Choice(val label: String, val tag: String, val marked: Boolean = false, val onClick: () -> Unit)

/** Ventanita con una fila por opción, la segura marcada. Tocar fuera = la marcada (`onDismiss`). */
@Composable
fun ChoiceDialog(
    title: String,
    choices: List<Choice>,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    tag: String = "choice-dialog",
) {
    val p = LocalPalette.current
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.surface,
        title = { Text(title, color = p.fg, fontSize = FontSizes.dialogTitle) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.gap)) {
                for (c in choices) {
                    val m = Modifier
                        .fillMaxWidth()
                        .testTag(c.tag)
                    if (c.marked) {
                        Button(
                            onClick = c.onClick,
                            colors = ButtonDefaults.buttonColors(containerColor = p.accent, contentColor = p.bg),
                            modifier = m,
                        ) { Text(c.label) }
                    } else {
                        OutlinedButton(
                            onClick = c.onClick,
                            border = BorderStroke(Dimens.rowBorder, p.dim),
                            modifier = m,
                        ) { Text(c.label, color = p.fg) }
                    }
                }
            }
        },
        confirmButton = {},
        modifier = modifier.testTag(tag),
    )
}

/** «¿Borrar …?»-style: dos filas, «No» marcado por defecto. */
@Composable
fun ConfirmDialog(
    title: String,
    yesLabel: String,
    onYes: () -> Unit,
    onNo: () -> Unit,
    noLabel: String = Txt.deleteNo,
    tag: String = "confirm-dialog",
) {
    ChoiceDialog(
        title = title,
        choices = listOf(
            Choice(yesLabel, "confirm-yes", onClick = onYes),
            Choice(noLabel, "confirm-no", marked = true, onClick = onNo),
        ),
        onDismiss = onNo,
        tag = tag,
    )
}
