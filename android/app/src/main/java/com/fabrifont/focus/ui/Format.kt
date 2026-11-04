package com.fabrifont.focus.ui

fun formatClock(secs: Long): String {
    val s = secs.coerceAtLeast(0)
    val h = s / 3600
    val m = s % 3600 / 60
    val ss = s % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, ss) else "%02d:%02d".format(m, ss)
}
