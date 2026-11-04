package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.Layout
import com.fabrifont.focus.ui.RoutineAction
import com.fabrifont.focus.ui.RoutinesModel
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.ActionSheet
import com.fabrifont.focus.ui.parts.ConfirmDialog
import com.fabrifont.focus.ui.parts.Hint
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle
import com.fabrifont.focus.ui.parts.SheetAction

/** Configuración › Rutinas: acá se crea, edita y borra. `onEdit(null)` = rutina nueva. */
@Composable
fun RoutinesConfigScreen(model: RoutinesModel, onEdit: (Int?) -> Unit, onBack: () -> Unit) {
    var sheetFor by rememberSaveable { mutableStateOf<Int?>(null) }
    var deleting by rememberSaveable { mutableStateOf<Int?>(null) }
    ScreenColumn {
        ScreenTitle(Txt.routinesTitle, onBack)
        if (model.broken) Hint(Txt.routinesBroken, tag = "routines-broken")
        MenuRow(Txt.routinesNew, enabled = !model.broken, tag = "routines-new") { onEdit(null) }
        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall),
            contentPadding = PaddingValues(top = Dimens.gap),
        ) {
            itemsIndexed(model.routines, key = { _, r -> r.name }) { i, r ->
                ListRow(r.name, tag = "routine-${r.name}", subtitle = Txt.routineTasks(r.tasks.size)) {
                    sheetFor = i
                }
            }
        }
    }
    val routine = sheetFor?.let { model.routines.getOrNull(it) }
    if (sheetFor != null && routine != null) {
        val i = sheetFor!!
        ActionSheet(
            tag = "routine-sheet",
            actions = Layout.routineActions.map { a ->
                when (a) {
                    RoutineAction.Edit -> SheetAction(Txt.routineEdit, "routine-edit") {
                        sheetFor = null
                        onEdit(i)
                    }
                    RoutineAction.Delete -> SheetAction(Txt.routineDelete, "routine-delete") {
                        sheetFor = null
                        deleting = i
                    }
                }
            },
            onDismiss = { sheetFor = null },
        )
    }
    val doomed = deleting?.let { model.routines.getOrNull(it) }
    if (deleting != null && doomed != null) {
        ConfirmDialog(
            title = Txt.deleteRoutineTitle(doomed.name),
            yesLabel = Txt.deleteYes,
            onYes = {
                model.save(model.routines.filterIndexed { i, _ -> i != deleting })
                deleting = null
            },
            onNo = { deleting = null },
        )
    }
}
