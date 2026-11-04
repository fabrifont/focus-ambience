package com.fabrifont.focus.ui

/** Todos los textos visibles de la app, en un solo lugar. */
object Txt {
    // Inicio
    const val appName = "focus"
    const val homeContinue = "Continuar"
    fun homeContinueRoutine(name: String) = "Continuar · $name"
    const val homeChooseRoutine = "Elegir otra rutina"
    const val homeFocus = "Ir a Focus"
    const val homeSettings = "Configuración"

    // Genéricos
    const val back = "‹"
    const val soon = "próximamente"
    const val cancel = "Cancelar"
    const val ok = "Aceptar"
    const val noName = "(sin nombre)"

    // Configuración
    const val settingsTitle = "Configuración"
    const val settingsSounds = "Sonidos ambientales"
    const val settingsPresets = "Presets"
    const val settingsRoutines = "Rutinas"
    const val settingsMeditations = "Meditaciones"

    // Configuración › Sonidos ambientales
    const val soundsTitle = "Sonidos ambientales"
    const val soundsAdd = "+ Agregar desde YouTube"
    const val soundsNoDelete = "Los sonidos generados no se pueden borrar"
    const val soundsBack = "Volver"

    // Focus
    const val focusOptions = "⋯ Opciones"
    const val clockEmpty = "--:--"
    const val clockPausedHint = "timer en pausa · tocá para seguir"

    // Antes de arrancar
    const val preStartTitle = "Antes de arrancar"
    fun preStartTitleStep(step: Int, total: Int) = "Antes de arrancar · $step de $total"
    const val preStartMinutes = "¿De cuántos minutos?"
    const val preStartNext = "Seguir"
    const val preStartNoTimer = "Entrar sin timer"
    const val preStartPickPreset = "Sonidos: elegí un preset"
    const val preStartCustom = "Sonidos personalizados"
    const val preStartGo = "Arrancar"
    const val preStartSkip = "Entrar sin arrancar nada"

    // Opciones y Timer
    const val optTimer = "Timer"
    const val optLoadPreset = "Cargar preset"
    const val optMaster = "Volumen general"
    const val optPause = "Pausar todo"
    const val optResume = "Seguir todo"
    const val optStop = "Apagar todo"
    const val optHome = "Volver al inicio"
    const val timerBack = "‹ Volver"
    fun timerMinutes(m: Int) = "$m min"
    const val timerCustom = "Personalizado…"
    fun timerStart(m: Int) = "Empezar $m min"
    const val timerCancel = "Cancelar timer"
    const val minus5 = "−5"
    const val plus5 = "+5"
    const val minutesSuffix = "min"

    // Cargar preset
    const val loadPresetTitle = "‹ Cargar preset"
    const val presetsEmpty = "Todavía no hay presets. Se crean en Configuración."

    // Configuración › Presets
    const val presetsTitle = "Presets"
    const val presetsNew = "+ Nuevo preset"
    fun presetSounds(n: Int) = if (n == 1) "1 sonido" else "$n sonidos"
    fun deletePresetTitle(name: String) = "¿Borrar «$name»?"
    const val deleteNo = "No"
    const val deleteYes = "Sí, borrar"

    // Nuevo preset
    const val builderTitle = "Nuevo preset"
    const val builderHint = "Elegí los sonidos y su volumen (suenan en vivo)"
    const val builderSave = "✓ Guardar preset"
    const val builderNameTitle = "Nombre de la mezcla"
    const val builderNameSave = "Guardar"
    const val builderNameTaken = "Ya hay un preset con ese nombre"
    const val builderNameEmpty = "Poné un nombre"
    const val builderDiscardTitle = "¿Descartar esta mezcla?"
    const val builderDiscardYes = "Sí, descartar"
    const val builderKeep = "Seguir armando"

    // Elegir rutina / Rutinas
    const val chooseTitle = "Elegir rutina"
    const val routinesTitle = "Rutinas"
    const val routinesNew = "+ Nueva rutina"
    const val routinesEmpty = "Todavía no hay rutinas. Se crean en Configuración."
    const val routinesBroken = "routines.toml está roto: no se puede cambiar hasta arreglarlo."
    const val routinesNone = "Sin rutinas"
    fun routineTasks(n: Int) = if (n == 1) "1 tarea" else "$n tareas"
    const val routineStart = "▶"
    const val routineEdit = "Editar"
    const val routineDelete = "Borrar"
    fun deleteRoutineTitle(name: String) = "¿Borrar «$name»?"

    // Editor de rutina
    const val editorTitleNew = "Nueva rutina"
    const val editorTitleEdit = "Editar rutina"
    fun editorName(name: String) = "Nombre: $name"
    const val editorNameTitle = "Nombre de la rutina"
    const val editorAddTask = "+ Agregar tarea"
    const val editorNewTaskTitle = "Nueva tarea"
    const val editorEditTaskTitle = "Editar texto"
    const val editorMeditation = "Meditación · próximamente"
    const val editorSave = "✓ Guardar"
    const val editorNameEmpty = "Poné un nombre a la rutina"
    const val editorNameTaken = "Ya hay una rutina con ese nombre"
    const val editorSaveFailed = "No se pudo guardar la rutina"
    fun editorTask(n: Int, text: String) = "$n  $text"
    const val taskEditText = "Editar texto"
    const val taskMove = "Mover"
    const val taskDelete = "Borrar"
    const val moveUp = "↑"
    const val moveDown = "↓"
    const val moveDone = "Listo"
    const val editorDiscardTitle = "Hay cambios sin guardar"
    const val editorSaveChoice = "Guardar"
    const val editorDiscardChoice = "Descartar"
    const val editorKeepEditing = "Seguir editando"

    // Rutina en curso
    fun runProgress(done: Int, total: Int) = "$done de $total listos"
    const val runNoTasks = "Esta rutina no tiene tareas"
    const val runToFocus = "Ir a Focus"
}
