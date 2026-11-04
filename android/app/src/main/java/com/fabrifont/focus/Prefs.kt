package com.fabrifont.focus

import android.content.Context

class Prefs(context: Context) {
    private val sp = context.getSharedPreferences("focus", Context.MODE_PRIVATE)

    /** Último tiempo de trabajo elegido, en minutos. */
    var lastMinutes: Int
        get() = sp.getInt("last_minutes", 25)
        set(v) = sp.edit().putInt("last_minutes", v).apply()
}
