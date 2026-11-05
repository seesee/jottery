package com.jottery.android.network

import com.jottery.android.model.AccountCredentials
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.concurrent.CopyOnWriteArrayList

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class AccountApiTest {
    private val requests = CopyOnWriteArrayList<RecordedRequest>()
    private var postStatus = 200
    private val credentials = AccountCredentials("user+tag@example.com", "account-password")

    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                requests += request
                return when {
                    request.method == "POST" && request.path == "/api/v1/user/status" ->
                        MockResponse().setResponseCode(postStatus)
                            .setBody("""{"exists":true,"isApproved":true,"isActive":true}""")
                    request.method == "GET" && request.path.orEmpty().startsWith("/api/v1/user/status") ->
                        MockResponse().setBody("""{"exists":true,"isApproved":false,"isActive":true}""")
                    request.path == "/api/v1/user/login" ->
                        MockResponse().setBody("""{"sessionId":"session","expiresAt":"t","mustChangePassword":false}""")
                    request.path == "/api/v1/user/account" ->
                        MockResponse().setBody(
                            """{"email":"e","noteCount":3,"attachmentCount":1,"storageUsedBytes":2048,
                               "storageQuotaMb":100,"createdAt":"t","lastSyncAt":null,
                               "inbox":{"itemCount":0,"totalSizeBytes":0,"maxItems":1,"maxSizeMb":1,"hasToken":true}}"""
                        )
                    else -> MockResponse().setBody("{}")
                }
            }
        }
        start()
    }

    private val client = SyncClient(server.url("/").toString().trimEnd('/'), "device-key", "client")

    @After fun tearDown() = server.shutdown()

    @Test fun `status proves the password rather than asking about an address`() = runBlocking {
        assertTrue(client.userStatus(credentials).isApproved)

        assertEquals("POST", requests.single().method)
    }

    @Test fun `an older server falls back to the GET, with the address escaped`() = runBlocking {
        postStatus = 405

        assertFalse(client.userStatus(credentials).isApproved)

        assertTrue(requests.last().path!!.contains("user%2Btag%40example.com"))
    }

    @Test fun `account calls use the session, never the device's key`() = runBlocking {
        val session = client.logIn(credentials).sessionId!!
        val info = client.accountInfo(session)
        client.deleteAccount(session, "deactivate")
        client.deleteServerNotes(session)

        assertEquals(3, info.noteCount)
        assertEquals(100L, info.storageQuotaMb)
        assertTrue(info.inbox.hasToken)
        requests.drop(1).forEach { assertEquals("Bearer session", it.getHeader("Authorization")) }
        assertEquals("/api/v1/user/account?mode=deactivate", requests[2].path)
    }
}
