package com.fabrifont.focus

import android.Manifest
import androidx.compose.ui.test.junit4.createEmptyComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class FocusFlowTest {
    @get:Rule
    val compose = createEmptyComposeRule()

    @Test
    fun pinkNoiseStartsPlaybackService() {
        val inst = InstrumentationRegistry.getInstrumentation()
        // Sin el diálogo de permiso encima de la app.
        runCatching {
            inst.uiAutomation.grantRuntimePermission(inst.targetContext.packageName, Manifest.permission.POST_NOTIFICATIONS)
        }

        ActivityScenario.launch(MainActivity::class.java).use {
            compose.onNodeWithTag("home-focus").performClick()
            compose.onNodeWithTag("prestart-notimer").performClick()
            compose.waitUntil(5_000) {
                compose.onAllNodesWithTagCount("switch-noise-pink") > 0
            }
            compose.onNodeWithTag("switch-noise-pink").performClick()
            compose.waitUntil(10_000) { PlaybackService.running }
            assertTrue(PlaybackService.running)

            // Apagar todo: el servicio se detiene solo.
            compose.onNodeWithTag("options").performClick()
            compose.waitUntil(5_000) { compose.onAllNodesWithTagCount("opt-stop") > 0 }
            compose.onNodeWithTag("opt-stop").performClick()
            compose.waitUntil(10_000) { !PlaybackService.running }
        }
    }
}

private fun androidx.compose.ui.test.junit4.ComposeTestRule.onAllNodesWithTagCount(tag: String): Int =
    onAllNodesWithTag(tag).fetchSemanticsNodes().size
