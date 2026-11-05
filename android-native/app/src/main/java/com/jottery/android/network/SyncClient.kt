package com.jottery.android.network

import com.jottery.android.model.*
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.decodeFromStream
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
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

    /**
     * Pull one page. The response can hold many blobs as base64, so it is decoded
     * from the stream rather than read into a string first.
     */
    @OptIn(ExperimentalSerializationApi::class)
    suspend fun pull(request: SyncPullRequest): SyncPullResponse {
        val body = json.encodeToString(request).toRequestBody(jsonMediaType)
        val httpRequest = buildAuthRequest("/api/v1/sync/pull").post(body).build()
        return client.newCall(httpRequest).execute().use { response ->
            if (!response.isSuccessful) throw failure(response)
            val stream = response.body?.byteStream() ?: throw SyncClientException("Empty response body")
            json.decodeFromStream(stream)
        }
    }

    /** A blob a pull didn't send. A caller must have checked [id] is a UUID. */
    @OptIn(ExperimentalSerializationApi::class)
    suspend fun getAttachment(id: String): SyncAttachment {
        val httpRequest = buildAuthRequest("/api/v1/sync/attachments/$id").get().build()
        return client.newCall(httpRequest).execute().use { response ->
            if (!response.isSuccessful) throw failure(response)
            val stream = response.body?.byteStream() ?: throw SyncClientException("Empty response body")
            json.decodeFromStream(stream)
        }
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

    // MARK: - Accounts (session, not API key)

    suspend fun registerUser(credentials: AccountCredentials): RegisterUserResponse {
        return postUnauthenticated("/api/v1/auth/register-user", credentials)
    }

    /**
     * Whether an account is approved. The POST proves the password and tells a
     * stranger nothing, so it's preferred. Older servers only have the GET.
     */
    suspend fun userStatus(credentials: AccountCredentials): UserStatusResponse {
        return try {
            postUnauthenticated("/api/v1/user/status", credentials)
        } catch (e: SyncClientException) {
            if (e.statusCode != 404 && e.statusCode != 405) throw e
            val url = "$baseUrl/api/v1/user/status".toHttpUrl().newBuilder()
                .addQueryParameter("email", credentials.email)
                .build()
            decoded(client.newCall(Request.Builder().url(url).get().build()).execute())
        }
    }

    suspend fun logIn(credentials: AccountCredentials): UserLoginResponse {
        return postUnauthenticated("/api/v1/user/login", credentials)
    }

    suspend fun logOut(sessionId: String) {
        send(sessionRequest("/api/v1/user/logout", sessionId).post(EMPTY_BODY))
    }

    suspend fun accountInfo(sessionId: String): UserAccountInfo {
        return decoded(client.newCall(sessionRequest("/api/v1/user/account", sessionId).get().build()).execute())
    }

    /** [mode] is `deactivate` or `delete`. */
    suspend fun deleteAccount(sessionId: String, mode: String) {
        send(sessionRequest("/api/v1/user/account?mode=$mode", sessionId).delete())
    }

    suspend fun deleteServerNotes(sessionId: String) {
        send(sessionRequest("/api/v1/user/notes", sessionId).delete())
    }

    suspend fun generateInboxToken(sessionId: String): InboxTokenResponse {
        val request = sessionRequest("/api/v1/user/inbox-token", sessionId).post(EMPTY_BODY).build()
        return decoded(client.newCall(request).execute())
    }

    suspend fun revokeInboxToken(sessionId: String) {
        send(sessionRequest("/api/v1/user/inbox-token", sessionId).delete())
    }

    private fun sessionRequest(path: String, sessionId: String): Request.Builder =
        Request.Builder().url("$baseUrl$path").header("Authorization", "Bearer $sessionId")

    private fun send(request: Request.Builder) {
        client.newCall(request.build()).execute().use { response ->
            if (!response.isSuccessful) throw failure(response)
        }
    }

    // MARK: - Wrapped Key (Envelope Encryption)

    /** Fetch the server-stored wrapped master key. Returns null if no key exists (404). */
    suspend fun getWrappedKey(): WrappedKeyResponse? {
        return try {
            get("/api/v1/auth/wrapped-key")
        } catch (e: SyncClientException) {
            if (e.statusCode == 404) null else throw e
        }
    }

    /**
     * Upload a wrapped master key to the server. With [ifMatch] it lands only if
     * the key is still the one that value came with. A write that's refused gets 409.
     */
    suspend fun putWrappedKey(request: PutWrappedKeyRequest, ifMatch: String? = null) {
        val jsonBody = json.encodeToString(request).toRequestBody(jsonMediaType)
        val builder = buildAuthRequest("/api/v1/auth/wrapped-key").put(jsonBody)
        ifMatch?.let { builder.header("If-Match", it) }
        client.newCall(builder.build()).execute().use { response ->
            if (!response.isSuccessful) throw failure(response)
        }
    }

    /** Remove this device from the account. */
    suspend fun revokeThisDevice() {
        delete("/api/v1/sync/device")
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
        return decoded(client.newCall(request).execute())
    }

    private inline fun <reified T, reified R> post(path: String, payload: T): R {
        val jsonBody = json.encodeToString(payload).toRequestBody(jsonMediaType)
        val request = buildAuthRequest(path).post(jsonBody).build()
        return decoded(client.newCall(request).execute())
    }

    private inline fun <reified T, reified R> postUnauthenticated(path: String, payload: T): R {
        val jsonBody = json.encodeToString(payload).toRequestBody(jsonMediaType)
        val request = buildUnauthRequest(path).post(jsonBody).build()
        return decoded(client.newCall(request).execute())
    }

    private inline fun <reified R> decoded(response: Response): R = response.use {
        if (!it.isSuccessful) throw failure(it)
        val body = it.body?.string() ?: throw SyncClientException("Empty response body")
        json.decodeFromString(body)
    }

    private fun delete(path: String) {
        val request = buildAuthRequest(path).delete().build()
        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) throw failure(response)
        }
    }

    /** Errors look like `{ error, code?, retryAfter? }`, but none of that is guaranteed. */
    private fun failure(response: Response): SyncClientException {
        val body = try {
            response.body?.string().orEmpty()
        } catch (_: java.io.IOException) {
            ""
        }
        val parsed = try {
            json.decodeFromString<ErrorBody>(body)
        } catch (_: Exception) {
            null
        }
        val retryAfter = response.header("Retry-After")?.trim()?.toLongOrNull() ?: parsed?.retryAfter
        return SyncClientException(
            message = "HTTP ${response.code}",
            statusCode = response.code,
            retryAfterSeconds = retryAfter?.takeIf { it > 0 }?.coerceAtMost(MAX_RETRY_AFTER_SECONDS),
            code = parsed?.code?.take(MAX_CODE_LENGTH),
            // Shown to the user, and the server is not trusted
            serverMessage = parsed?.error?.filterNot { it.isISOControl() }?.take(MAX_MESSAGE_LENGTH),
        )
    }

    @Serializable
    private data class ErrorBody(
        val error: String? = null,
        val code: String? = null,
        val retryAfter: Long? = null,
    )

    private companion object {
        val EMPTY_BODY = ByteArray(0).toRequestBody()
        const val MAX_RETRY_AFTER_SECONDS = 3600L
        const val MAX_MESSAGE_LENGTH = 300
        const val MAX_CODE_LENGTH = 64
    }
}

class SyncClientException(
    message: String,
    val statusCode: Int? = null,
    /** How long the server asked us to wait, from a 429. */
    val retryAfterSeconds: Long? = null,
    /** The server's stable error code, if it sent one. */
    val code: String? = null,
    /** The server's own wording, cleaned for display. */
    val serverMessage: String? = null,
) : Exception(message)
