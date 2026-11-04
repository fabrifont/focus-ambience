package com.fabrifont.focus

import android.Manifest
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineHost

private const val WAIT_MS = 10_000L

fun ComposeTestRule.has(tag: String): Boolean = onAllNodesWithTag(tag).fetchSemanticsNodes().isNotEmpty()

fun ComposeTestRule.waitForTag(tag: String) = waitUntil(WAIT_MS) { has(tag) }

fun ComposeTestRule.waitGone(tag: String) = waitUntil(WAIT_MS) { !has(tag) }

/** Espera el tag, lo toca. */
fun ComposeTestRule.tap(tag: String) {
    waitForTag(tag)
    onNodeWithTag(tag).performClick()
}

/** Cada test arranca limpio: sin sonido ni timer, y sin las cosas de CI en la biblioteca. */
fun resetForTest(presets: List<String> = emptyList(), routines: List<String> = emptyList()) {
    val inst = InstrumentationRegistry.getInstrumentation()
    runCatching {
        inst.uiAutomation.grantRuntimePermission(inst.targetContext.packageName, Manifest.permission.POST_NOTIFICATIONS)
    }
    val engine = EngineHost.get(inst.targetContext)
    engine.send(EngineCommand.StopAll)
    engine.send(EngineCommand.Timer(null))
    for (p in presets) engine.send(EngineCommand.DeletePreset(p))
    runCatching { engine.saveRoutines(engine.routines().filter { it.name !in routines }) }
    runCatching { engine.setLastRoutine(null) }
}

/** Captura de pantalla para el PR: queda en files/capturas de la app y el CI la baja con adb pull. */
fun ComposeTestRule.shot(name: String) {
    waitForIdle()
    val inst = InstrumentationRegistry.getInstrumentation()
    runCatching {
        val dir = java.io.File(inst.targetContext.getExternalFilesDir(null), "capturas").apply { mkdirs() }
        val bmp = inst.uiAutomation.takeScreenshot() ?: return
        java.io.FileOutputStream(java.io.File(dir, "$name.png")).use {
            bmp.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
        }
    }
}
