package com.fabrifont.focus.ui.parts

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.theme.LocalPalette

/** Campo de texto en una ventanita. `error` se muestra bajo el campo (lo decide quien llama). */
@Composable
fun TextInputDialog(
    title: String,
    initial: String,
    confirmLabel: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
    error: String? = null,
) {
    val p = LocalPalette.current
    var text by rememberSaveable { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.surface,
        title = { Text(title, color = p.accent, fontSize = FontSizes.dialogTitle) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                singleLine = true,
                isError = error != null,
                supportingText = error?.let { e -> { Text(e) } },
                modifier = Modifier.testTag("input-field"),
            )
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(text.trim()) }, modifier = Modifier.testTag("input-ok")) {
                Text(confirmLabel, color = p.accent)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, modifier = Modifier.testTag("input-cancel")) {
                Text(Txt.cancel, color = p.fg)
            }
        },
        modifier = Modifier.testTag("input-dialog"),
    )
}
