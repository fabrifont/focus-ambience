package com.fabrifont.focus.engine

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Process
import android.os.SystemClock
import java.util.concurrent.Executors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.focus_ffi.Command
import uniffi.focus_ffi.CoreException
import uniffi.focus_ffi.Listener
import uniffi.focus_ffi.Player
import uniffi.focus_ffi.State

// TODO(core): cuando exista core/, este motor pasa a envolver sus bindings en vez de focus_ffi.

private const val SAMPLE_RATE = 48_000
private const val CHUNK_FRAMES = 2_400 // 50 ms
private const val FADE_TAIL_MS = 1_200L // lo que tarda en bajar el volumen tras pausar
private const val PUBLISH_EVERY_MS = 500L
private const val IDLE_TICK_MS = 250L

class RustEngine(context: Context) : FocusEngine {
    private val player = Player(context.filesDir.path)
    private val _state = MutableStateFlow(player.state().toUi())
    override val state: StateFlow<EngineState> = _state.asStateFlow()

    @Volatile
    override var timerFinishedCallback: (() -> Unit)? = null

    // Los comandos van por un hilo aparte: guardar volúmenes escribe a disco.
    private val commands = Executors.newSingleThreadExecutor { Thread(it, "focus-cmd") }

    @Volatile
    private var audioThread: Thread? = null

    init {
        player.setListener(object : Listener {
            override fun onState(state: State) {
                _state.value = state.toUi()
            }

            override fun onTimerFinished() {
                timerFinishedCallback?.invoke()
            }
        })
    }

    override fun send(cmd: EngineCommand) {
        val c = cmd.toCore()
        commands.execute {
            try {
                player.send(c)
            } catch (_: CoreException) {
                // Sonido desconocido: nada que hacer.
            }
        }
    }

    @Synchronized
    override fun startAudio() {
        if (audioThread != null) return
        val t = Thread(::audioLoop, "focus-audio")
        audioThread = t
        t.start()
    }

    @Synchronized
    override fun stopAudio() {
        val t = audioThread ?: return
        audioThread = null
        t.interrupt()
        t.join(2_000)
    }

    private fun buildTrack(): AudioTrack {
        val format = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
            .setSampleRate(SAMPLE_RATE)
            .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO)
            .build()
        val min = AudioTrack.getMinBufferSize(SAMPLE_RATE, AudioFormat.CHANNEL_OUT_STEREO, AudioFormat.ENCODING_PCM_FLOAT)
        val bytes = maxOf(min, CHUNK_FRAMES * 2 * 4 * 3)
        return AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build(),
            )
            .setAudioFormat(format)
            .setBufferSizeInBytes(bytes)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()
    }

    private fun audioLoop() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        val track = buildTrack()
        val buf = FloatArray(CHUNK_FRAMES * 2)
        var playing = false
        var lastAudible = Long.MIN_VALUE / 2
        var lastPublish = 0L
        try {
            while (!Thread.currentThread().isInterrupted) {
                val now = SystemClock.elapsedRealtime()
                if (_state.value.audible) lastAudible = now
                if (now - lastAudible < FADE_TAIL_MS) {
                    if (!playing) {
                        track.play()
                        playing = true
                    }
                    val out = player.render(CHUNK_FRAMES.toUInt())
                    for (i in buf.indices) buf[i] = out[i]
                    track.write(buf, 0, buf.size, AudioTrack.WRITE_BLOCKING)
                } else {
                    if (playing) {
                        track.pause()
                        track.flush()
                        playing = false
                    }
                    player.tick()
                    Thread.sleep(IDLE_TICK_MS)
                }
                if (now - lastPublish >= PUBLISH_EVERY_MS) {
                    _state.value = player.state().toUi()
                    lastPublish = now
                }
            }
        } catch (_: InterruptedException) {
            // stopAudio()
        } finally {
            track.release()
        }
    }
}

private fun State.toUi() = EngineState(
    sounds = sounds.map { SoundItem(it.id, it.name, it.theme) },
    volumes = volumes,
    active = active.toSet(),
    paused = paused,
    master = master,
    timer = timer?.let { TimerInfo(it.totalSecs.toLong(), it.remainingSecs.toLong(), it.paused) },
)

private fun EngineCommand.toCore(): Command = when (this) {
    is EngineCommand.SetActive -> Command.SetActive(id, active)
    is EngineCommand.Toggle -> Command.Toggle(id)
    is EngineCommand.SetVolume -> Command.SetVolume(id, volume)
    is EngineCommand.SetMaster -> Command.SetMaster(volume)
    EngineCommand.PauseAll -> Command.PauseAll
    EngineCommand.StopAll -> Command.StopAll
    is EngineCommand.Timer -> Command.Timer(minutes?.toUInt())
    EngineCommand.ToggleTimerPause -> Command.ToggleTimerPause
}
