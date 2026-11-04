package com.fabrifont.focus.ui.screens

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.fabrifont.focus.engine.RoutineItem
import com.fabrifont.focus.ui.Dimens
import com.fabrifont.focus.ui.EditorEntry
import com.fabrifont.focus.ui.Layout
import com.fabrifont.focus.ui.RoutinesModel
import com.fabrifont.focus.ui.TaskAction
import com.fabrifont.focus.ui.Txt
import com.fabrifont.focus.ui.parts.ActionSheet
import com.fabrifont.focus.ui.parts.BottomButton
import com.fabrifont.focus.ui.parts.Choice
import com.fabrifont.focus.ui.parts.ChoiceDialog
import com.fabrifont.focus.ui.parts.Hint
import com.fabrifont.focus.ui.parts.ListRow
import com.fabrifont.focus.ui.parts.MenuRow
import com.fabrifont.focus.ui.parts.ScreenColumn
import com.fabrifont.focus.ui.parts.ScreenTitle
import com.fabrifont.focus.ui.parts.SheetAction
import com.fabrifont.focus.ui.parts.TextInputDialog
import com.fabrifont.focus.ui.theme.LocalPalette

/** Qué ventanita de texto está abierta: nombre, tarea nueva o tarea `index`. */
private sealed interface Typing {
    data object Name : Typing
    data object NewTask : Typing
    data class Task(val index: Int) : Typing
}

private val StringListSaver = listSaver<SnapshotStateList<String>, String>(
    save = { it.toList() },
    restore = { it.toMutableStateList() },
)

/** Editor de rutina (solo se entra desde Configuración). `index` null = rutina nueva. */
@Composable
fun RoutineEditorScreen(model: RoutinesModel, index: Int?, onClose: () -> Unit) {
    val original = index?.let { model.routines.getOrNull(it) }
    var name by rememberSaveable { mutableStateOf(original?.name ?: "") }
    val tasks = rememberSaveable(saver = StringListSaver) { (original?.tasks ?: emptyList()).toMutableStateList() }
    var typing by rememberSaveable { mutableStateOf<Int?>(null) } // -1 nombre, -2 tarea nueva, >=0 tarea
    var taskSheet by rememberSaveable { mutableStateOf<Int?>(null) }
    var moving by rememberSaveable { mutableStateOf<Int?>(null) }
    var leaving by rememberSaveable { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    var nameError by rememberSaveable { mutableStateOf<String?>(null) }

    val dirty = name != (original?.name ?: "") || tasks.toList() != (original?.tasks ?: emptyList<String>())

    fun mode(): Typing? = when (val t = typing) {
        null -> null
        -1 -> Typing.Name
        -2 -> Typing.NewTask
        else -> Typing.Task(t)
    }

    /** Valida y guarda; `true` si salió bien (y entonces cierra). */
    fun save(): Boolean {
        val n = name.trim()
        val problem = when {
            n.isEmpty() -> Txt.editorNameEmpty
            model.nameTaken(n, index) -> Txt.editorNameTaken
            else -> null
        }
        if (problem != null) {
            nameError = problem
            error = problem
            typing = -1
            return false
        }
        val item = RoutineItem(n, tasks.toList(), original?.meditation)
        val list = if (index != null && original != null) {
            model.routines.mapIndexed { i, r -> if (i == index) item else r }
        } else {
            model.routines + item
        }
        if (!model.save(list)) {
            error = Txt.editorSaveFailed
            return false
        }
        onClose()
        return true
    }

    fun back() {
        when {
            moving != null -> moving = null
            dirty -> leaving = true
            else -> onClose()
        }
    }
    BackHandler(onBack = ::back)

    ScreenColumn {
        ScreenTitle(if (index == null) Txt.editorTitleNew else Txt.editorTitleEdit, onBack = ::back)
        LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(Dimens.gapSmall)) {
            for (entry in Layout.editor) when (entry) {
                EditorEntry.Name -> item {
                    MenuRow(
                        Txt.editorName(name.ifBlank { Txt.noName }),
                        tag = "editor-name",
                    ) {
                        nameError = null
                        typing = -1
                    }
                }
                EditorEntry.Tasks -> itemsIndexed(tasks) { i, text ->
                    ListRow(
                        Txt.editorTask(i + 1, text),
                        tag = "editor-task-$i",
                        highlight = moving == i,
                    ) { if (moving == null) taskSheet = i }
                }
                EditorEntry.AddTask -> item {
                    MenuRow(Txt.editorAddTask, tag = "editor-add-task") { typing = -2 }
                }
                EditorEntry.Meditation -> item {
                    MenuRow(Txt.editorMeditation, enabled = false, tag = "editor-meditation")
                }
            }
        }
        error?.let { Hint(it, modifier = Modifier.padding(top = Dimens.gapSmall), tag = "editor-error") }
        val m = moving
        if (m == null) {
            BottomButton(Txt.editorSave, tag = "editor-save") { save() }
        } else {
            MoveBar(
                canUp = m > 0,
                canDown = m < tasks.lastIndex,
                onUp = {
                    tasks.add(m - 1, tasks.removeAt(m))
                    moving = m - 1
                },
                onDown = {
                    tasks.add(m + 1, tasks.removeAt(m))
                    moving = m + 1
                },
                onDone = { moving = null },
            )
        }
    }

    mode()?.let { t ->
        val title = when (t) {
            Typing.Name -> Txt.editorNameTitle
            Typing.NewTask -> Txt.editorNewTaskTitle
            is Typing.Task -> Txt.editorEditTaskTitle
        }
        val initial = when (t) {
            Typing.Name -> name
            Typing.NewTask -> ""
            is Typing.Task -> tasks.getOrElse(t.index) { "" }
        }
        TextInputDialog(
            title = title,
            initial = initial,
            confirmLabel = Txt.ok,
            error = if (t == Typing.Name) nameError else null,
            onDismiss = { typing = null },
            onConfirm = { text ->
                when (t) {
                    Typing.Name -> {
                        name = text
                        nameError = null
                        error = null
                        typing = null
                    }
                    Typing.NewTask -> {
                        if (text.isNotEmpty()) tasks.add(text)
                        typing = null
                    }
                    is Typing.Task -> {
                        if (text.isNotEmpty() && t.index in tasks.indices) tasks[t.index] = text
                        typing = null
                    }
                }
            },
        )
    }
    taskSheet?.let { i ->
        if (i in tasks.indices) {
            ActionSheet(
                tag = "task-sheet",
                actions = Layout.taskActions.map { a ->
                    when (a) {
                        TaskAction.EditText -> SheetAction(Txt.taskEditText, "task-edit") {
                            taskSheet = null
                            typing = i
                        }
                        TaskAction.Move -> SheetAction(Txt.taskMove, "task-move") {
                            taskSheet = null
                            moving = i
                        }
                        TaskAction.Delete -> SheetAction(Txt.taskDelete, "task-delete") {
                            taskSheet = null
                            tasks.removeAt(i)
                        }
                    }
                },
                onDismiss = { taskSheet = null },
            )
        }
    }
    if (leaving) {
        ChoiceDialog(
            title = Txt.editorDiscardTitle,
            choices = listOf(
                Choice(Txt.editorSaveChoice, "leave-save") {
                    leaving = false
                    save()
                },
                Choice(Txt.editorDiscardChoice, "leave-discard") {
                    leaving = false
                    onClose()
                },
                Choice(Txt.editorKeepEditing, "leave-keep", marked = true) { leaving = false },
            ),
            onDismiss = { leaving = false },
            tag = "leave-dialog",
        )
    }
}

@Composable
private fun MoveBar(canUp: Boolean, canDown: Boolean, onUp: () -> Unit, onDown: () -> Unit, onDone: () -> Unit) {
    val p = LocalPalette.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = Dimens.bottomBarTop),
        horizontalArrangement = Arrangement.spacedBy(Dimens.gap),
    ) {
        OutlinedButton(onClick = onUp, enabled = canUp, modifier = Modifier.testTag("move-up")) {
            Text(Txt.moveUp, color = p.accent)
        }
        OutlinedButton(onClick = onDown, enabled = canDown, modifier = Modifier.testTag("move-down")) {
            Text(Txt.moveDown, color = p.accent)
        }
        BottomButton(Txt.moveDone, tag = "move-done", modifier = Modifier.weight(1f), onClick = onDone)
    }
}
