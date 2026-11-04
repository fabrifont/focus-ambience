package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.fabrifont.focus.engine.RoutineItem
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.Hint
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle

/** Elegir rutina desde Inicio: tocar una la empieza. Nada de editar, borrar ni crear. */
@Composable
fun ChooseRoutineScreen(routines: List<RoutineItem>, onStart: (RoutineItem) -> Unit, onBack: () -> Unit) {
    ScreenColumn {
        ScreenTitle(Txt.chooseTitle, onBack)
        if (routines.isEmpty()) Hint(Txt.routinesEmpty, tag = "choose-empty")
        LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall)) {
            items(routines, key = { it.name }) { r ->
                ListRow(
                    r.name,
                    tag = "choose-${r.name}",
                    subtitle = Txt.routineTasks(r.tasks.size),
                    trailing = Txt.routineStart,
                ) { onStart(r) }
            }
        }
    }
}
