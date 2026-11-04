package com.fabrifont.focus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.RoutineItem
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.FontSizes
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.BottomButton
import com.fabrifont.focus.ui.parts.Hint
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle
import com.fabrifont.focus.ui.theme.LocalPalette

private val DoneSaver = listSaver<SnapshotStateList<Int>, Int>(
    save = { it.toList() },
    restore = { it.toMutableStateList() },
)

/**
 * Rutina en curso: se tildan las tareas. Al completar todas pasa a Focus (`onToFocus`, igual que el
 * botón de abajo); `onBack` sale sin terminar. El ambiente lo pausa/devuelve quien navega (FocusApp).
 */
@Composable
fun RoutineRunScreen(routine: RoutineItem, onBack: () -> Unit, onToFocus: () -> Unit) {
    val p = LocalPalette.current
    val done = rememberSaveable(saver = DoneSaver) { mutableListOf<Int>().toMutableStateList() }
    val total = routine.tasks.size
    val count = done.size

    LaunchedEffect(count, total) {
        if (total > 0 && count == total) onToFocus()
    }

    ScreenColumn {
        ScreenTitle(routine.name, onBack)
        Text(
            Txt.runProgress(count, total),
            color = p.accent2,
            fontSize = FontSizes.body,
            modifier = Modifier
                .testTag("run-progress")
                .padding(bottom = Dimens.gap),
        )
        if (total == 0) Hint(Txt.runNoTasks)
        LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall)) {
            itemsIndexed(routine.tasks) { i, text ->
                val checked = i in done
                ListRow(
                    text,
                    tag = "run-task-$i",
                    leading = if (checked) "☑" else "☐",
                    highlight = false,
                ) { if (checked) done.remove(i) else done.add(i) }
            }
        }
        BottomButton(Txt.runToFocus, tag = "run-to-focus", onClick = onToFocus)
    }
}
