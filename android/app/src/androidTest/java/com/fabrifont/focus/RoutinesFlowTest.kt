package com.fabrifont.focus

import androidx.compose.ui.test.junit4.createEmptyComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTextInput
import androidx.test.core.app.ActivityScenario
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class RoutinesFlowTest {
    @get:Rule
    val compose = createEmptyComposeRule()

    private val name = "Mañana CI"

    @Before
    fun clean() = resetForTest(routines = listOf(name))

    @After
    fun cleanAfter() = resetForTest(routines = listOf(name))

    private fun type(openTag: String, text: String) {
        compose.tap(openTag)
        compose.waitForTag("input-field")
        compose.onNodeWithTag("input-field").performTextInput(text)
        compose.tap("input-ok")
        compose.waitGone("input-dialog")
    }

    @Test
    fun createRunAndDeleteRoutine() {
        ActivityScenario.launch(MainActivity::class.java).use {
            // Configuración › Rutinas › + Nueva rutina
            compose.tap("home-settings")
            compose.tap("settings-routines")
            compose.tap("routines-new")
            type("editor-name", name)
            type("editor-add-task", "Uno")
            type("editor-add-task", "Dos")
            compose.shot("e2-editor-rutina")
            compose.tap("editor-save")
            compose.waitForTag("routine-$name")
            compose.shot("e2-config-rutinas")

            // Inicio › Elegir otra rutina: tocar empieza.
            compose.tap("back")
            compose.tap("back")
            compose.tap("home-routines")
            compose.shot("e2-elegir-rutina")
            compose.tap("choose-$name")
            compose.tap("run-task-0")
            compose.shot("e2-rutina-en-curso")
            compose.tap("run-task-1")

            // Al completar pasa a Focus (pasando por «Antes de arrancar» si no hay sesión).
            compose.waitUntil(10_000) { compose.has("prestart") || compose.has("clock") }
            if (compose.has("prestart")) compose.tap("prestart-notimer")
            compose.waitForTag("clock")

            // Volver al inicio: «Continuar» aparece con la última rutina; después se borra.
            compose.tap("options")
            compose.tap("opt-home")
            compose.waitForTag("home-continue")
            compose.shot("e2-inicio")
            assertTrue(compose.has("home-continue"))
            compose.tap("home-settings")
            compose.tap("settings-routines")
            compose.tap("routine-$name")
            compose.tap("routine-delete")
            compose.waitForTag("confirm-no")
            compose.tap("confirm-yes")
            compose.waitGone("routine-$name")
        }
    }
}
