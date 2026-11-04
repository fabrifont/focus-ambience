package com.fabrifont.focus.ui.parts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.theme.LocalPalette

/** Hoja desde abajo con la columna de filas de siempre. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SheetScaffold(
    tag: String,
    onDismiss: () -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    val p = LocalPalette.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = p.surface,
        modifier = Modifier.testTag(tag),
    ) {
        Column(
            modifier = Modifier
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(horizontal = Dimens.sheetH, vertical = Dimens.sheetV),
            verticalArrangement = Arrangement.spacedBy(Dimens.gap),
            content = content,
        )
    }
}

data class SheetAction(val label: String, val tag: String, val onClick: () -> Unit)

/** Hoja con una lista corta de acciones. */
@Composable
fun ActionSheet(tag: String, actions: List<SheetAction>, onDismiss: () -> Unit) {
    SheetScaffold(tag, onDismiss) {
        for (a in actions) MenuRow(a.label, tag = a.tag, onClick = a.onClick)
    }
}
