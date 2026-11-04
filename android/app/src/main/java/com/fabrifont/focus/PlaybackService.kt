package com.fabrifont.focus

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.media.app.NotificationCompat.MediaStyle
import com.fabrifont.focus.engine.EngineCommand
import com.fabrifont.focus.engine.EngineState
import com.fabrifont.focus.engine.FocusEngine
import com.fabrifont.focus.engine.EngineHost
import com.fabrifont.focus.ui.formatClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

private const val CHANNEL_PLAYBACK = "playback"
private const val CHANNEL_TIMER_DONE = "timer_done"
private const val ID_PLAYBACK = 1
private const val ID_TIMER_DONE = 2
private const val WAKE_LOCK_MS = 8 * 60 * 60 * 1000L

/** Lo único que se ve en la notificación; se actualiza solo si cambia. */
private data class NotifModel(
    val title: String,
    val text: String,
    val total: Int,
    val elapsed: Int,
    val hasTimer: Boolean,
    val running: Boolean,
)

private fun EngineState.toNotif(): NotifModel {
    val names = sounds.filter { it.id in active }.joinToString(" · ") { it.name }
    val t = timer
    val text = when {
        t != null -> formatClock(t.remainingSecs) + " restantes" + if (running) "" else " · en pausa"
        running -> "Sonando"
        else -> "En pausa"
    }
    return NotifModel(
        title = names.ifEmpty { "focus" },
        text = text,
        total = (t?.totalSecs ?: 0).toInt(),
        elapsed = t?.let { (it.totalSecs - it.remainingSecs).toInt() } ?: 0,
        hasTimer = t != null,
        running = running,
    )
}

/**
 * Mantiene vivo el proceso mientras suena algo o corre el timer. Dueño del audio del motor,
 * la notificación, la sesión de medios y el foco de audio.
 */
class PlaybackService : Service() {
    companion object {
        const val ACTION_TOGGLE = "com.fabrifont.focus.TOGGLE"

        /** Para el test instrumentado y para no arrancar el servicio dos veces. */
        @Volatile
        var running = false
            private set

        fun start(context: Context) {
            ContextCompat.startForegroundService(context, Intent(context, PlaybackService::class.java))
        }
    }

    private lateinit var engine: FocusEngine
    private lateinit var session: MediaSessionCompat
    private lateinit var notifications: NotificationManager
    private lateinit var audio: AudioManager
    private var wakeLock: PowerManager.WakeLock? = null
    private var focusRequest: AudioFocusRequest? = null
    private var pausedByFocus = false
    private var last: NotifModel? = null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    private val noisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            pauseAudible(byFocus = false)
        }
    }

    override fun onCreate() {
        super.onCreate()
        running = true
        engine = EngineHost.get(this)
        notifications = getSystemService(NotificationManager::class.java)
        audio = getSystemService(AudioManager::class.java)
        createChannels()

        session = MediaSessionCompat(this, "focus").apply {
            setCallback(object : MediaSessionCompat.Callback() {
                override fun onPlay() = setRunning(true)
                override fun onPause() = setRunning(false)
            })
            isActive = true
        }

        val model = engine.state.value.toNotif()
        last = model
        ServiceCompat.startForeground(
            this, ID_PLAYBACK, buildNotification(model),
            if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
        )

        @Suppress("WakelockTimeout")
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "focus:playback")
            .apply { acquire(WAKE_LOCK_MS) }

        ContextCompat.registerReceiver(
            this, noisyReceiver, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        requestFocus()
        engine.timerFinishedCallback = ::onTimerFinished
        engine.startAudio()

        scope.launch {
            engine.state.map { it.toNotif() }.distinctUntilChanged().collect { update(it) }
        }
        scope.launch {
            engine.state.map { it.audible }.distinctUntilChanged().collect { if (it) pausedByFocus = false }
        }
        scope.launch {
            // Sin nada prendido ni timer (Apagar todo, apagar el último sonido) no hay para qué seguir.
            engine.state.map { it.busy }.distinctUntilChanged().collect { if (!it) shutDown() }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_TOGGLE) engine.send(EngineCommand.PauseAll)
        // Cada startForegroundService exige un startForeground, aunque ya estuviéramos en primer plano.
        last?.let {
            ServiceCompat.startForeground(
                this, ID_PLAYBACK, buildNotification(it),
                if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
            )
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        running = false
        scope.cancel()
        engine.timerFinishedCallback = null
        engine.stopAudio()
        unregisterReceiver(noisyReceiver)
        focusRequest?.let { audio.abandonAudioFocusRequest(it) }
        session.isActive = false
        session.release()
        wakeLock?.takeIf { it.isHeld }?.release()
        super.onDestroy()
    }

    private fun shutDown() {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun setRunning(wanted: Boolean) {
        if (engine.state.value.running != wanted) engine.send(EngineCommand.PauseAll)
    }

    // ---------- foco de audio ----------

    private fun requestFocus() {
        val req = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build(),
            )
            .setOnAudioFocusChangeListener({ change ->
                when (change) {
                    AudioManager.AUDIOFOCUS_LOSS,
                    AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> pauseAudible(byFocus = true)
                    AudioManager.AUDIOFOCUS_GAIN -> if (pausedByFocus) {
                        pausedByFocus = false
                        if (engine.state.value.paused) engine.send(EngineCommand.PauseAll)
                    }
                }
            }, Handler(Looper.getMainLooper()))
            .build()
        focusRequest = req
        audio.requestAudioFocus(req)
    }

    private fun pauseAudible(byFocus: Boolean) {
        if (!engine.state.value.audible) return
        pausedByFocus = byFocus
        engine.send(EngineCommand.PauseAll)
    }

    // ---------- notificación ----------

    private fun createChannels() {
        notifications.createNotificationChannel(
            NotificationChannel(CHANNEL_PLAYBACK, "Reproducción", NotificationManager.IMPORTANCE_LOW)
                .apply { setShowBadge(false) },
        )
        notifications.createNotificationChannel(
            NotificationChannel(CHANNEL_TIMER_DONE, "Timer terminado", NotificationManager.IMPORTANCE_LOW),
        )
    }

    private fun update(model: NotifModel) {
        last = model
        session.setPlaybackState(
            PlaybackStateCompat.Builder()
                .setActions(
                    PlaybackStateCompat.ACTION_PLAY or PlaybackStateCompat.ACTION_PAUSE or
                        PlaybackStateCompat.ACTION_PLAY_PAUSE,
                )
                .setState(
                    if (model.running) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED,
                    PlaybackStateCompat.PLAYBACK_POSITION_UNKNOWN, 1f,
                )
                .build(),
        )
        notifications.notify(ID_PLAYBACK, buildNotification(model))
    }

    private fun buildNotification(m: NotifModel): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val toggle = PendingIntent.getService(
            this, 1, Intent(this, PlaybackService::class.java).setAction(ACTION_TOGGLE),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val b = NotificationCompat.Builder(this, CHANNEL_PLAYBACK)
            .setSmallIcon(R.drawable.ic_stat_focus)
            .setContentTitle(m.title)
            .setContentText(m.text)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_TRANSPORT)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .addAction(
                if (m.running) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play,
                if (m.running) "Pausar" else "Seguir",
                toggle,
            )
            .setStyle(MediaStyle().setMediaSession(session.sessionToken).setShowActionsInCompactView(0))
        if (m.hasTimer) b.setProgress(m.total, m.elapsed, false)
        return b.build()
    }

    private fun onTimerFinished() {
        // Decisión 9 de los wireframes: en el celu no hay aviso al terminar, solo el fade-out.
    }
}
