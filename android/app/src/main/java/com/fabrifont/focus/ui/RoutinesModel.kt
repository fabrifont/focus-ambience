package com.fabrifont.focus.ui

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.fabrifont.focus.engine.FocusEngine
import com.fabrifont.focus.engine.RoutineItem

/**
 * Las rutinas tal como están en `routines.toml` (el archivo lo maneja core). Leer y escribir
 * un toml chico es barato, así que va en el hilo de la UI. Si el archivo está roto no se pisa:
 * `broken` bloquea los cambios.
 */
@Stable
class RoutinesModel(private val engine: FocusEngine) {
    var routines by mutableStateOf<List<RoutineItem>>(emptyList())
        private set
    var broken by mutableStateOf(false)
        private set
    private var last by mutableStateOf<String?>(null)

    init {
        reload()
    }

    fun reload() {
        try {
            routines = engine.routines()
            broken = false
        } catch (e: Exception) {
            routines = emptyList()
            broken = true
        }
        last = runCatching { engine.lastRoutine() }.getOrNull()
    }

    /** La última rutina usada, solo si todavía existe. */
    val continueName: String? get() = last?.takeIf { l -> routines.any { it.name == l } }

    fun get(name: String?): RoutineItem? = routines.firstOrNull { it.name == name }

    /** Guarda la lista entera; `false` si el archivo está roto o falló la escritura. */
    fun save(list: List<RoutineItem>): Boolean {
        if (broken) return false
        return try {
            engine.saveRoutines(list)
            routines = list
            true
        } catch (e: Exception) {
            false
        }
    }

    fun nameTaken(name: String, skip: Int?): Boolean =
        runCatching { engine.routineNameTaken(routines, name, skip) }.getOrDefault(false)

    fun markStarted(name: String) {
        runCatching { engine.setLastRoutine(name) }
        last = name
    }
}
