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

    // MARK: - Device Registration

    suspend fun registerDevice(request: RegisterDeviceRequest): RegisterDeviceResponse {
        return post("/api/devices/register", request)
    }

    suspend fun cloneDevice(request: CloneDeviceRequest): RegisterDeviceResponse {
        return post("/api/devices/clone", request)
    }

    // MARK: - Sync

    suspend fun push(request: SyncPushRequest): SyncPushResponse {
        return post("/api/sync/push", request)
    }

    suspend fun pull(request: SyncPullRequest): SyncPullResponse {
        return post("/api/sync/pull", request)
    }

    suspend fun status(): SyncStatusResponse {
        return get("/api/sync/status")
    }

    // MARK: - SSE Token

    suspend fun getSSEToken(): SSETokenResponse {
        return get("/api/sync/sse-token")
    }

    // MARK: - Inbox

    suspend fun getInboxItems(): List<InboxItem> {
        return get("/api/inbox/items")
    }

    suspend fun getInboxStatus(): InboxStatusResponse {
        return get("/api/inbox/status")
    }

    suspend fun deleteInboxItem(id: String) {
        delete("/api/inbox/items/$id")
    }

    suspend fun deleteAllInboxItems() {
        delete("/api/inbox/items")
    }

    suspend fun getInboxToken(): InboxTokenResponse {
        return get("/api/inbox/token")
    }

    suspend fun getInboxTokenStatus(): InboxTokenStatusResponse {
        return get("/api/inbox/token/status")
    }

    // MARK: - Private HTTP Methods

    private inline fun <reified T> buildAuthRequest(path: String): Request.Builder {
        return Request.Builder()
            .url("$baseUrl$path")
            .header("Authorization", "Bearer $apiKey")
            .header("X-Client-ID", clientId)
            .header("Content-Type", "application/json")
    }

    private inline fun <reified R> get(path: String): R {
        val request = buildAuthRequest<Unit>(path).get().build()
        val response = client.newCall(request).execute()
        val body = response.body?.string() ?: throw SyncClientException("Empty response body")
        if (!response.isSuccessful) {
            throw SyncClientException("HTTP ${response.code}: $body")
        }
        return json.decodeFromString(body)
    }

    private inline fun <reified T, reified R> post(path: String, payload: T): R {
        val jsonBody = json.encodeToString(payload).toRequestBody(jsonMediaType)
        val request = buildAuthRequest<Unit>(path).post(jsonBody).build()
        val response = client.newCall(request).execute()
        val body = response.body?.string() ?: throw SyncClientException("Empty response body")
        if (!response.isSuccessful) {
            throw SyncClientException("HTTP ${response.code}: $body")
        }
        return json.decodeFromString(body)
    }

    private fun delete(path: String) {
        val request = buildAuthRequest<Unit>(path).delete().build()
        val response = client.newCall(request).execute()
        if (!response.isSuccessful) {
            val body = response.body?.string() ?: ""
            throw SyncClientException("HTTP ${response.code}: $body")
        }
    }
}

class SyncClientException(message: String) : Exception(message)
