package com.jottery.android

import android.app.Application
import com.jottery.android.database.JotteryDatabase

/**
 * Application class. Initialises the database singleton.
 */
class JotteryApp : Application() {

    lateinit var database: JotteryDatabase
        private set

    override fun onCreate() {
        super.onCreate()
        instance = this
        database = JotteryDatabase.getInstance(this)
    }

    companion object {
        lateinit var instance: JotteryApp
            private set
    }
}
