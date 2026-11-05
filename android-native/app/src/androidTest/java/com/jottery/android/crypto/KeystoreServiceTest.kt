package com.jottery.android.crypto

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID

/** Against the real Keystore, which is the whole point of this class. */
@RunWith(AndroidJUnit4::class)
class KeystoreServiceTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val legacyName = "jottery_secure_prefs"
    private val prefsFolder = File(context.applicationInfo.dataDir, "shared_prefs")
    private val legacyFile = File(prefsFolder, "$legacyName.xml")
    private val sealedFile = File(prefsFolder, "jottery_sealed_prefs.xml")

    private fun legacyPrefs() = EncryptedSharedPreferences.create(
        context,
        legacyName,
        MasterKey.Builder(context).setKeyScheme(MasterKey.KeyScheme.AES256_GCM).build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    private fun clearAll() {
        KeystoreService.deleteAPIKey(context)
        KeystoreService.deleteClientId(context)
        context.deleteSharedPreferences(legacyName)
        KeystoreService.forgetLegacyCheck()
    }

    @Before fun setUp() = clearAll()
    @After fun tearDown() = clearAll()

    @Test fun aValueComesBackAndCanBeDeleted() {
        val value = UUID.randomUUID().toString()

        KeystoreService.setAPIKey(context, value)
        assertEquals(value, KeystoreService.getAPIKey(context))

        KeystoreService.deleteAPIKey(context)
        assertNull(KeystoreService.getAPIKey(context))
    }

    @Test fun whatReachesTheDiskIsNotTheValue() {
        val value = UUID.randomUUID().toString()

        context.getSharedPreferences("jottery_sealed_prefs", Context.MODE_PRIVATE).edit().commit()
        KeystoreService.setAPIKey(context, value)
        context.getSharedPreferences("jottery_sealed_prefs", Context.MODE_PRIVATE).edit().commit()

        assertTrue(sealedFile.exists())
        assertFalse(sealedFile.readText().contains(value))
    }

    @Test fun anInstallUpgradingKeepsItsConnection() {
        val apiKey = UUID.randomUUID().toString()
        val clientId = UUID.randomUUID().toString()
        legacyPrefs().edit().putString("api_key", apiKey).putString("client_id", clientId).commit()
        assertTrue(legacyFile.exists())

        assertEquals(apiKey, KeystoreService.getAPIKey(context))
        assertEquals(clientId, KeystoreService.getClientId(context))

        assertFalse("the old copy shouldn't linger", legacyFile.exists())
        KeystoreService.forgetLegacyCheck()
        assertEquals("and it lasts past this run", apiKey, KeystoreService.getAPIKey(context))
    }

    @Test fun aNewerValueIsNotReplacedByTheOldFile() {
        val newer = UUID.randomUUID().toString()
        KeystoreService.setAPIKey(context, newer)
        legacyPrefs().edit().putString("api_key", "older").commit()
        KeystoreService.forgetLegacyCheck()

        assertEquals(newer, KeystoreService.getAPIKey(context))
    }

    @Test fun anOldFileThatWontOpenIsGivenUpOn() {
        prefsFolder.mkdirs()
        legacyFile.writeText("<map><string name=\"not\">valid</string></map>")
        context.deleteSharedPreferences("__androidx_security_crypto_encrypted_prefs_key_keyset__")

        assertNull(KeystoreService.getAPIKey(context))

        assertFalse(legacyFile.exists())
    }
}
