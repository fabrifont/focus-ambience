package com.fabrifont.focus.ui

/** Entradas del menú de Inicio. */
enum class HomeEntry { Continue, ChooseRoutine, Focus, Settings }

/** Entradas de Configuración. */
enum class SettingsEntry { Sounds, Presets, Routines, Meditations }

/** Filas de la hoja Opciones (los hijos de «Timer ›», «Cargar preset ›»…). */
enum class OptionsEntry { Timer, LoadPreset, Master, Pause, Stop, Home }

/** Acciones de una rutina en Configuración. */
enum class RoutineAction { Edit, Delete }

/** Acciones de una tarea en el editor. */
enum class TaskAction { EditText, Move, Delete }

/** Filas del editor de rutina antes de la lista de tareas / después. El «✓ Guardar» va fijo abajo. */
enum class EditorEntry { Name, Tasks, AddTask, Meditation }

/** Orden de todo lo que se elige de una lista fija; para reordenar, tocar solo acá. */
object Layout {
    val home = listOf(HomeEntry.Continue, HomeEntry.ChooseRoutine, HomeEntry.Focus, HomeEntry.Settings)
    val settings = listOf(SettingsEntry.Sounds, SettingsEntry.Presets, SettingsEntry.Routines, SettingsEntry.Meditations)
    val options = listOf(
        OptionsEntry.Timer, OptionsEntry.LoadPreset, OptionsEntry.Master,
        OptionsEntry.Pause, OptionsEntry.Stop, OptionsEntry.Home,
    )
    val routineActions = listOf(RoutineAction.Edit, RoutineAction.Delete)
    val taskActions = listOf(TaskAction.EditText, TaskAction.Move, TaskAction.Delete)
    val editor = listOf(EditorEntry.Name, EditorEntry.Tasks, EditorEntry.AddTask, EditorEntry.Meditation)

    /** Minutos de los atajos de la hoja Timer. */
    val timerShortcuts = listOf(25, 50, 90)
}
