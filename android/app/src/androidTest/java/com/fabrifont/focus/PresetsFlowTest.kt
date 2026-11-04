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

class PresetsFlowTest {
    @get:Rule
    val compose = createEmptyComposeRule()

    private val name = "Prueba CI"

    @Before
    fun clean() = resetForTest(presets = listOf(name))

    @After
    fun cleanAfter() = resetForTest(presets = listOf(name))

    @Test
    fun createLoadAndDeletePreset() {
        ActivityScenario.launch(MainActivity::class.java).use {
            // Configuración › Presets › + Nuevo preset
            compose.tap("home-settings")
            compose.tap("settings-presets")
            compose.tap("presets-new")
            compose.tap("switch-noise-pink")
            compose.shot("e2-nuevo-preset")
            compose.tap("builder-save")
            compose.waitForTag("input-field")
            compose.onNodeWithTag("input-field").performTextInput(name)
            compose.tap("input-ok")
            compose.waitForTag("preset-$name")
            compose.shot("e2-config-presets")

            // Focus › Opciones › Cargar preset
            compose.tap("back")
            compose.tap("back")
            compose.tap("home-focus")
            compose.tap("prestart-notimer")
            compose.tap("options")
            compose.tap("opt-preset")
            compose.shot("e2-cargar-preset")
            compose.tap("load-preset-$name")
            compose.waitUntil(10_000) { PlaybackService.running }
            assertTrue(PlaybackService.running)

            // Apagar todo y borrar desde Configuración.
            compose.tap("options")
            compose.tap("opt-stop")
            compose.waitUntil(10_000) { !PlaybackService.running }
            compose.tap("options")
            compose.tap("opt-home")
            compose.tap("home-settings")
            compose.tap("settings-presets")
            compose.tap("preset-$name")
            compose.waitForTag("confirm-no")
            compose.shot("e2-borrar-preset")
            compose.tap("confirm-yes")
            compose.waitGone("preset-$name")
        }
    }
}
