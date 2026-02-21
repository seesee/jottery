package com.jottery.android.network

import com.jottery.android.model.*
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/**
 * HTTP client for the Jottery sync API.
 * Uses OkHttp with the device's API key for authentication.
 */
class SyncClient(
    private val baseUrl: String,
    private val apiKey: String,
    private val clientId: String,
) {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .writeTimeout(60, TimeUnit.SECONDS)
        .build()

    private val jsonMediaType = "application/json; charset=utf-8".toMediaType()

    // MARK: - Device Registration (unauthenticated)

    suspend fun registerDevice(request: RegisterDeviceRequest): RegisterDeviceResponse {
        return postUnauthenticated("/api/v1/auth/register-device", request)
    }

    suspend fun cloneDevice(request: CloneDeviceRequest): RegisterDeviceResponse {
        return postUnauthenticated("/api/v1/auth/clone-device", request)
    }

    // MARK: - Sync (authenticated)

    suspend fun push(request: SyncPushRequest): SyncPushResponse {
        return post("/api/v1/sync/push", request)
    }

    suspend fun pull(request: SyncPullRequest): SyncPullResponse {
        return post("/api/v1/sync/pull", request)
    }

    suspend fun status(): SyncStatusResponse {
        return get("/api/v1/sync/status")
    }

    // MARK: - SSE Token

    suspend fun getSSEToken(): SSETokenResponse {
        return get("/api/v1/sync/events/token")
    }

    // MARK: - Inbox

    suspend fun getInboxItems(): List<InboxItem> {
        return get("/api/v1/inbox")
    }

    suspend fun getInboxStatus(): InboxStatusResponse {
        return get("/api/v1/inbox/status")
    }

    suspend fun deleteInboxItem(id: String) {
        delete("/api/v1/inbox/$id")
    }

    suspend fun deleteAllInboxItems() {
        delete("/api/v1/inbox")
    }

    suspend fun getInboxToken(): InboxTokenResponse {
        return get("/api/v1/user/inbox-token")
    }

    suspend fun getInboxTokenStatus(): InboxTokenStatusResponse {
        return get("/api/v1/user/inbox-token/status")
    }

    // MARK: - Private HTTP Methods

    private fun buildAuthRequest(path: String): Request.Builder {
        return Request.Builder()
            .url("$baseUrl$path")
            .header("Authorization", "Bearer $apiKey")
            .header("X-Client-ID", clientId)
            .header("Content-Type", "application/json")
    }

    private fun buildUnauthRequest(path: String): Request.Builder {
        return Request.Builder()
            .url("$baseUrl$path")
            .header("Content-Type", "application/json")
    }

    private inline fun <reified R> get(path: String): R {
        val request = buildAuthRequest(path).get().build()
        val response = client.newCall(request).execute()
        val body = response.body?.string() ?: throw SyncClientException("Empty response body")
        if (!response.isSuccessful) {
            throw SyncClientException("HTTP ${response.code}: $body")
        }
        return json.decodeFromString(body)
    }

    private inline fun <reified T, reified R> post(path: String, payload: T): R {
        val jsonBody = json.encodeToString(payload).toRequestBody(jsonMediaType)
        val request = buildAuthRequest(path).post(jsonBody).build()
        val response = client.newCall(request).execute()
        val body = response.body?.string() ?: throw SyncClientException("Empty response body")
        if (!response.isSuccessful) {
            throw SyncClientException("HTTP ${response.code}: $body")
        }
        return json.decodeFromString(body)
    }

    private inline fun <reified T, reified R> postUnauthenticated(path: String, payload: T): R {
        val jsonBody = json.encodeToString(payload).toRequestBody(jsonMediaType)
        val request = buildUnauthRequest(path).post(jsonBody).build()
        val response = client.newCall(request).execute()
        val body = response.body?.string() ?: throw SyncClientException("Empty response body")
        if (!response.isSuccessful) {
            throw SyncClientException("HTTP ${response.code}: $body")
        }
        return json.decodeFromString(body)
    }

    private fun delete(path: String) {
        val request = buildAuthRequest(path).delete().build()
        val response = client.newCall(request).execute()
        if (!response.isSuccessful) {
            val body = response.body?.string() ?: ""
            throw SyncClientException("HTTP ${response.code}: $body")
        }
    }
}

class SyncClientException(message: String) : Exception(message)
