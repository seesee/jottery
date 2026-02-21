package com.jottery.android.ui.theme

import androidx.compose.ui.graphics.Color

// Light theme
val md_theme_light_primary = Color(0xFF6750A4)
val md_theme_light_onPrimary = Color(0xFFFFFFFF)
val md_theme_light_primaryContainer = Color(0xFFEADDFF)
val md_theme_light_onPrimaryContainer = Color(0xFF21005E)
val md_theme_light_secondary = Color(0xFF625B71)
val md_theme_light_onSecondary = Color(0xFFFFFFFF)
val md_theme_light_secondaryContainer = Color(0xFFE8DEF8)
val md_theme_light_onSecondaryContainer = Color(0xFF1D192B)
val md_theme_light_surface = Color(0xFFFFFBFE)
val md_theme_light_onSurface = Color(0xFF1C1B1F)
val md_theme_light_surfaceVariant = Color(0xFFE7E0EC)
val md_theme_light_onSurfaceVariant = Color(0xFF49454F)
val md_theme_light_error = Color(0xFFB3261E)
val md_theme_light_onError = Color(0xFFFFFFFF)
val md_theme_light_background = Color(0xFFFFFBFE)
val md_theme_light_onBackground = Color(0xFF1C1B1F)
val md_theme_light_outline = Color(0xFF79747E)

// Dark theme
val md_theme_dark_primary = Color(0xFFD0BCFF)
val md_theme_dark_onPrimary = Color(0xFF381E72)
val md_theme_dark_primaryContainer = Color(0xFF4F378B)
val md_theme_dark_onPrimaryContainer = Color(0xFFEADDFF)
val md_theme_dark_secondary = Color(0xFFCCC2DC)
val md_theme_dark_onSecondary = Color(0xFF332D41)
val md_theme_dark_secondaryContainer = Color(0xFF4A4458)
val md_theme_dark_onSecondaryContainer = Color(0xFFE8DEF8)
val md_theme_dark_surface = Color(0xFF1C1B1F)
val md_theme_dark_onSurface = Color(0xFFE6E1E5)
val md_theme_dark_surfaceVariant = Color(0xFF49454F)
val md_theme_dark_onSurfaceVariant = Color(0xFFCAC4D0)
val md_theme_dark_error = Color(0xFFF2B8B5)
val md_theme_dark_onError = Color(0xFF601410)
val md_theme_dark_background = Color(0xFF1C1B1F)
val md_theme_dark_onBackground = Color(0xFFE6E1E5)
val md_theme_dark_outline = Color(0xFF938F99)

// Note colours matching iOS Color+NoteColors.swift
object NoteColors {
    val RED = Color(0xFFEF5350)
    val ORANGE = Color(0xFFFFA726)
    val YELLOW = Color(0xFFFFEE58)
    val GREEN = Color(0xFF66BB6A)
    val TEAL = Color(0xFF26A69A)
    val BLUE = Color(0xFF42A5F5)
    val PURPLE = Color(0xFFAB47BC)
    val PINK = Color(0xFFEC407A)

    val allColors = mapOf(
        "red" to RED,
        "orange" to ORANGE,
        "yellow" to YELLOW,
        "green" to GREEN,
        "teal" to TEAL,
        "blue" to BLUE,
        "purple" to PURPLE,
        "pink" to PINK,
    )

    fun fromString(name: String?): Color? = name?.let { allColors[it] }
}
